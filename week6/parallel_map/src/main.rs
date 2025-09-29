use crossbeam_channel;
use std::{sync::mpsc, thread, time};

fn parallel_map<T, U, F>(mut input_vec: Vec<T>, num_threads: usize, f: F) -> Vec<U>
where
    F: FnOnce(T) -> U + Send + Copy + 'static,
    T: Send + 'static,
    U: Send + 'static + Default,
{
    let len = input_vec.len();
    let mut output_vec: Vec<U> = Vec::with_capacity(len);
    output_vec.resize_with(input_vec.len(), Default::default);
    // Implement parallel map!
    let (s1, r1) = crossbeam_channel::unbounded::<(T, usize)>();
    let (tx, rx) = mpsc::channel::<(U, usize)>();

    let mut idx = len;
    while let Some(val) = input_vec.pop() {
        idx -= 1;
        s1.send((val, idx)).unwrap();
    }
    drop(s1);
    
    for _ in 0..num_threads {
        let r1 = r1.clone();
        let tx = tx.clone();
        thread::spawn(move || {
            while let Ok((val, idx)) = r1.recv() {
                tx.send((f(val), idx)).unwrap();
            }
        });
    }
    drop(tx);

    for (val, idx) in rx {
        output_vec[idx] = val;
    }

    output_vec
}

// Implement a parallelized Mandelbrot Set generator.
fn mandelbrot_escape(x: f64, y: f64, max_iter: usize) -> usize {
    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut iter = 0;
    while zx * zx + zy * zy < 4.0 && iter < max_iter {
        let tmp = zx * zx - zy * zy + x;
        zy = 2.0 * zx * zy + y;
        zx = tmp;
        iter += 1;
    }
    iter
}

fn main() {
    let width = 80;
    let height = 24;
    let max_iter = 20;
    let mut points = Vec::with_capacity(width * height);
    for j in 0..height {
        for i in 0..width {
            let x = (i as f64) * 3.0 / (width as f64) - 2.0;
            let y = (j as f64) * 2.0 / (height as f64) - 1.0;
            points.push((x, y));
        }
    }
    let results = parallel_map(points, 12, move |(x, y)| mandelbrot_escape(x, y, max_iter));
    for j in 0..height {
        for i in 0..width {
            let idx = j * width + i;
            let v = results[idx];
            let c = match v {
                0..=2 => ' ',
                3..=7 => '.',
                8..=15 => '*',
                16..=25 => 'o',
                26..=35 => 'O',
                36..=39 => '@',
                _ => '#',
            };
            print!("{}", c);
        }
        println!("");
    }
}
