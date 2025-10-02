mod request;
mod response;

use clap::Parser;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, VecDeque};

use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use std::io::{Error, ErrorKind};
use tokio::time::sleep;

use std::sync::Arc;

/// Contains information parsed from the command-line invocation of balancebeam. The Clap macros
/// provide a fancy way to automatically construct a command-line argument parser.
#[derive(Parser, Debug)]
#[command(about = "Fun with load balancing")]
struct CmdOptions {
    /// "IP/port to bind to"
    #[arg(short, long, default_value = "0.0.0.0:1100")]
    bind: String,
    /// "Upstream host to forward requests to"
    #[arg(short, long)]
    upstream: Vec<String>,
    /// "Perform active health checks on this interval (in seconds)"
    #[arg(long, default_value = "10")]
    active_health_check_interval: usize,
    /// "Path to send request to for active health checks"
    #[arg(long, default_value = "/")]
    active_health_check_path: String,
    /// "Maximum number of requests to accept per IP per minute (0 = unlimited)"
    #[arg(long, default_value = "0")]
    max_requests_per_minute: usize,
}

/// Contains information about the state of balancebeam (e.g. what servers we are currently proxying
/// to, what servers have failed, rate limiting counts, etc.)
///
/// You should add fields to this struct in later milestones.
struct ProxyState {
    /// How frequently we check whether upstream servers are alive (Milestone 4)
    #[allow(dead_code)]
    active_health_check_interval: usize,
    /// Where we should send requests when doing active health checks (Milestone 4)
    #[allow(dead_code)]
    active_health_check_path: String,
    /// Maximum number of requests an individual IP can make in a minute (Milestone 5)
    #[allow(dead_code)]
    max_requests_per_minute: usize,
    /// Addresses of servers that we are proxying to
    upstream_addresses: Vec<String>,
    /// Addresses of servers that are alive
    liveing_upstreams: RwLock<Vec<String>>,
    /// Map for rate limit count
    rate_sliding_window: Mutex<HashMap<String, VecDeque<Instant>>>,
}

#[tokio::main]
async fn main() {
    // Initialize the logging library. You can print log messages using the `log` macros:
    // https://docs.rs/log/0.4.8/log/ You are welcome to continue using print! statements; this
    // just looks a little prettier.
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "debug");
    }
    pretty_env_logger::init();

    // Parse the command line arguments passed to this program
    let options = CmdOptions::parse();
    if options.upstream.len() < 1 {
        log::error!("At least one upstream server must be specified using the --upstream option.");
        std::process::exit(1);
    }

    // Start listening for connections
    let listener = match TcpListener::bind(&options.bind).await {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("Could not bind to {}: {}", options.bind, err);
            std::process::exit(1);
        }
    };
    log::info!("Listening for requests on {}", options.bind);

    // Handle incoming connections
    let state = Arc::new(ProxyState {
        upstream_addresses: options.upstream.clone(),
        liveing_upstreams: RwLock::new(options.upstream),
        active_health_check_interval: options.active_health_check_interval,
        active_health_check_path: options.active_health_check_path,
        max_requests_per_minute: options.max_requests_per_minute,
        rate_sliding_window: Mutex::new(HashMap::new()),
    });

    let state_temp = Arc::clone(&state);
    tokio::spawn(async move {
        active_health_check(state_temp).await;
    });

    // Handle incoming connections.
    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(err) => {
                log::error!("accept error: {}", err);
                continue;
            }
        };
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            handle_connection(stream, state).await;
        });
    }
}

async fn active_health_check(state: Arc<ProxyState>) {
    loop {
        // Sleep for the configured interval before each probe round
        let interval_secs = state.active_health_check_interval as u64;
        sleep(Duration::from_secs(interval_secs)).await;

        let targets = state.upstream_addresses.clone();
        let mut healthy = Vec::with_capacity(targets.len());

        for upstream in targets {
            let req = http::Request::builder()
                .method(http::Method::GET)
                .uri(state.active_health_check_path.as_str())
                .header("Host", upstream.as_str())
                .body(Vec::new())
                .unwrap();

            match TcpStream::connect(&upstream).await {
                Ok(mut conn) => {
                    if let Err(err) = request::write_to_stream(&req, &mut conn).await {
                        log::error!("health check write to {} failed: {}", upstream, err);
                        continue;
                    }

                    let response = match response::read_from_stream(&mut conn, &req.method()).await {
                        Ok(r) => r,
                        Err(err) => {
                            log::error!("health check read from {} failed: {:?}", upstream, err);
                            continue;
                        }
                    };

                    if response.status().as_u16() == 200 {
                        healthy.push(upstream);
                    } else {
                        log::warn!(
                            "health check {} returned non-200 status: {}",
                            upstream,
                            response.status()
                        );
                    }
                }
                Err(err) => {
                    log::error!("health check connect to {} failed: {}", upstream, err);
                }
            }
        }

        let mut live = state.liveing_upstreams.write().await;
        *live = healthy;
    }
}

async fn rate_limiting_check(state: Arc<ProxyState>, client: &mut TcpStream) -> Result<(), Error> {
    let client_ip = client.peer_addr().unwrap().ip().to_string();

    let now = Instant::now();
    let window = Duration::from_secs(60);
    let cutoff = now - window;

    let mut map = state.rate_sliding_window.lock().await;
    let deque = map.entry(client_ip).or_insert(VecDeque::new());

    while matches!(deque.front(), Some(ts) if *ts < cutoff) {
        deque.pop_front();
    }

    if deque.len() >= state.max_requests_per_minute {
        let response = response::make_http_error(http::StatusCode::TOO_MANY_REQUESTS);
        if let Err(e) = response::write_to_stream(&response, client).await {
            log::warn!("Failed to send 429: {}", e);
        }
        return Err(Error::new(ErrorKind::Other, "Too many requests"));
    }

    deque.push_back(now);
    Ok(())
}

async fn connect_to_upstream(state: Arc<ProxyState>) -> Result<TcpStream, std::io::Error> {
    let mut rng = rand::rngs::StdRng::from_entropy();
    loop {
        let upstreams = state.liveing_upstreams.read().await;
        if upstreams.len() == 0 {
            break;
        }
        let upstream_idx = rng.gen_range(0..upstreams.len());
        let upstream_ip = upstreams[upstream_idx].clone();
        drop(upstreams);

        match TcpStream::connect(&upstream_ip).await {
            Ok(stream) => return Ok(stream),
            Err(_) => {
                let mut upstreams = state.liveing_upstreams.write().await;
                upstreams.remove(upstream_idx);
            }
        }
    }
    // Implement failover (milestone 3)
    Err(std::io::Error::new(
    std::io::ErrorKind::Other,
    "No available upstream servers",
    ))
}

async fn send_response(client_conn: &mut TcpStream, response: &http::Response<Vec<u8>>) {
    let client_ip = client_conn.peer_addr().unwrap().ip().to_string();
    log::info!(
        "{} <- {}",
        client_ip,
        response::format_response_line(&response)
    );
    if let Err(error) = response::write_to_stream(&response, client_conn).await {
        log::warn!("Failed to send response to client: {}", error);
        return;
    }
}

async fn handle_connection(mut client_conn: TcpStream, state: Arc<ProxyState>) {
    let client_ip = client_conn.peer_addr().unwrap().ip().to_string();
    log::info!("Connection received from {}", client_ip);

    // Open a connection to a random destination server
    let mut upstream_conn = match connect_to_upstream(Arc::clone(&state)).await {
        Ok(stream) => stream,
        Err(_error) => {
            let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
            send_response(&mut client_conn, &response).await;
            return;
        }
    };
    let upstream_ip = client_conn.peer_addr().unwrap().ip().to_string();

    // The client may now send us one or more requests. Keep trying to read requests until the
    // client hangs up or we get an error.
    loop {
        // Read a request from the client
        let mut request = match request::read_from_stream(&mut client_conn).await {
            Ok(request) => request,
            // Handle case where client closed connection and is no longer sending requests
            Err(request::Error::IncompleteRequest(0)) => {
                log::debug!("Client finished sending requests. Shutting down connection");
                return;
            }
            // Handle I/O error in reading from the client
            Err(request::Error::ConnectionError(io_err)) => {
                log::info!("Error reading request from client stream: {}", io_err);
                return;
            }
            Err(error) => {
                log::debug!("Error parsing request: {:?}", error);
                let response = response::make_http_error(match error {
                    request::Error::IncompleteRequest(_)
                    | request::Error::MalformedRequest(_)
                    | request::Error::InvalidContentLength
                    | request::Error::ContentLengthMismatch => http::StatusCode::BAD_REQUEST,
                    request::Error::RequestBodyTooLarge => http::StatusCode::PAYLOAD_TOO_LARGE,
                    request::Error::ConnectionError(_) => http::StatusCode::SERVICE_UNAVAILABLE,
                });
                send_response(&mut client_conn, &response).await;
                continue;
            }
        };
        log::info!(
            "{} -> {}: {}",
            client_ip,
            upstream_ip,
            request::format_request_line(&request)
        );

        if state.max_requests_per_minute > 0 {
            let state = Arc::clone(&state);
            if let Err(_) = rate_limiting_check(state, &mut client_conn).await {
                continue;
            }
        }

        // Add X-Forwarded-For header so that the upstream server knows the client's IP address.
        // (We're the ones connecting directly to the upstream server, so without this header, the
        // upstream server will only know our IP, not the client's.)
        request::extend_header_value(&mut request, "x-forwarded-for", &client_ip);

        // Forward the request to the server
        if let Err(error) = request::write_to_stream(&request, &mut upstream_conn).await {
            log::error!(
                "Failed to send request to upstream {}: {}",
                upstream_ip,
                error
            );
            let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
            send_response(&mut client_conn, &response).await;
            return;
        }
        log::debug!("Forwarded request to server");

        // Read the server's response
        let response = match response::read_from_stream(&mut upstream_conn, request.method()).await
        {
            Ok(response) => response,
            Err(error) => {
                log::error!("Error reading response from server: {:?}", error);
                let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
                send_response(&mut client_conn, &response).await;
                return;
            }
        };
        // Forward the response to the client
        send_response(&mut client_conn, &response).await;
        log::debug!("Forwarded response to client");
    }
}
