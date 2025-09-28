use std::{thread, time};

const NUM_THREADS: u32 = 20;

fn main() {
    let mut threads = Vec::new();
    println!("Spawning {} threads...", NUM_THREADS);
    for i in 0..NUM_THREADS {
        threads.push(thread::spawn(move || {
            let millis = i * 100;
            thread::sleep(time::Duration::from_millis(millis as u64));
            println!("Thread {} finished running!", i);
        }));
    }

    for handle in threads {
        handle.join().expect("Panic happend inside of a thread");
    }
    println!("All threads finished!");
}