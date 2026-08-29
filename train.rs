use std::env;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Messages that travel the train
#[derive(Debug)]
enum TrainMsg {
	Forward(Vec<u8>),
	Backward(String),
	Terminate,
}

/// One car = one external process defined in cars.txt
struct TrainCar {
	id: usize,
	cmd: String,
}

impl TrainCar {
	fn new(id: usize, cmd: String) -> Self {
		Self { id, cmd }
	}

	fn attach(
		self,
		rx_in: Receiver<TrainMsg>,
		tx_out: Sender<TrainMsg>,
		rx_back: Receiver<TrainMsg>,
		tx_back: Sender<TrainMsg>,
		is_last: bool,
		loop_mode: bool,
	) -> JoinHandle<()> {
		thread::spawn(move || {
			loop {
				// Prefer forward traffic
				match rx_in.recv_timeout(Duration::from_millis(10)) {
					Ok(TrainMsg::Forward(data)) => {
						println!(
							"Car {} ({}) received {} bytes",
							self.id,
							self.cmd,
							data.len()
						);

						// Spawn a fresh process for this chunk.
						// Closing stdin after the write gives the child EOF,
						// so read_to_end is guaranteed to return.
						let mut child = match Command::new("sh")
							.arg("-c")
							.arg(&self.cmd)
							.stdin(Stdio::piped())
							.stdout(Stdio::piped())
							.stderr(Stdio::inherit())
							.spawn()
						{
							Ok(c) => c,
							Err(e) => {
								let _ = tx_back.send(TrainMsg::Backward(format!(
									"Car {} failed to start '{}': {}",
									self.id, self.cmd, e
								)));
								continue;
							}
						};

						let mut stdin = child.stdin.take().expect("stdin");
						let mut stdout = child.stdout.take().expect("stdout");

						// Feed data then close stdin → child sees EOF
						if let Err(e) = stdin.write_all(&data) {
							let _ = tx_back.send(TrainMsg::Backward(format!(
								"Car {} write error: {}",
								self.id, e
							)));
							let _ = child.kill();
							continue;
						}
						drop(stdin); // critical: close the pipe

						// Now safe – process will eventually exit and close stdout
						let mut output = Vec::new();
						if let Err(e) = stdout.read_to_end(&mut output) {
							let _ = tx_back.send(TrainMsg::Backward(format!(
								"Car {} read error: {}",
								self.id, e
							)));
							let _ = child.kill();
							continue;
						}

						// Wait for the child so we don't leave zombies
						let _ = child.wait();

						// If the process produced nothing, pass original data through
						if output.is_empty() {
							output = data;
						}

						if is_last {
							if loop_mode {
								let _ = tx_back.send(TrainMsg::Forward(output));
							} else {
								let _ = tx_back.send(TrainMsg::Backward(format!(
									"Car {} ({}) finished – {} bytes",
									self.id,
									self.cmd,
									output.len()
								)));
							}
						} else if tx_out.send(TrainMsg::Forward(output)).is_err() {
							let _ = tx_back.send(TrainMsg::Backward(format!(
								"Car {} downstream closed",
								self.id
							)));
							break;
						}
					}
					Ok(TrainMsg::Terminate) => {
						let _ = tx_out.send(TrainMsg::Terminate);
						break;
					}
					Ok(TrainMsg::Backward(_)) => {}
					Err(mpsc::RecvTimeoutError::Timeout) => {}
					Err(mpsc::RecvTimeoutError::Disconnected) => break,
				}

				// Drain backward feedback
				while let Ok(msg) = rx_back.try_recv() {
					if let TrainMsg::Backward(fb) = msg {
						println!("Car {} passing feedback: {}", self.id, fb);
						let _ = tx_back.send(TrainMsg::Backward(fb));
					}
				}
			}
		})
	}
}

/// Engine that owns the chain
struct TrainEngine {
	car_handles: Vec<JoinHandle<()>>,
	engine_tx: Sender<TrainMsg>,
	engine_rx_back: Receiver<TrainMsg>,
}

impl TrainEngine {
	fn assemble(commands: Vec<String>, loop_mode: bool) -> Self {
		assert!(!commands.is_empty(), "cars.txt produced no commands");

		let n = commands.len();
		let (engine_tx, mut next_rx_in) = mpsc::channel();
		let (mut next_tx_back, engine_rx_back) = mpsc::channel();
		let mut car_handles = Vec::with_capacity(n);

		for (i, cmd) in commands.into_iter().enumerate() {
			let id = i + 1;
			let is_last = id == n;
			let (tx_out, rx_in) = mpsc::channel();
			let (tx_back, rx_back) = mpsc::channel();

			let car = TrainCar::new(id, cmd);
			let handle = car.attach(
				next_rx_in,
				tx_out,
				rx_back,
				next_tx_back,
				is_last,
				loop_mode,
			);
			car_handles.push(handle);

			next_rx_in = rx_in;
			next_tx_back = tx_back;
		}

		Self {
			car_handles,
			engine_tx,
			engine_rx_back,
		}
	}

	fn dispatch(&self, data: Vec<u8>) {
		let _ = self.engine_tx.send(TrainMsg::Forward(data));
	}

	fn terminate(&self) {
		let _ = self.engine_tx.send(TrainMsg::Terminate);
	}
}

/// Load cars.txt → list of commands
fn load_cars(path: &str) -> io::Result<Vec<String>> {
	let file = fs::File::open(path)?;
	let reader = io::BufReader::new(file);
	let mut cmds = Vec::new();
	for line in reader.lines() {
		let line = line?;
		let trimmed = line.trim();
		if trimmed.is_empty() || trimmed.starts_with('#') {
			continue;
		}
		cmds.push(trimmed.to_string());
	}
	Ok(cmds)
}

fn main() {
	let args: Vec<String> = env::args().collect();
	let loop_mode = args.iter().any(|a| a.to_lowercase().contains("loop"));
	let cars_file = args
		.iter()
		.find(|a| a.ends_with(".txt"))
		.map(|s| s.as_str())
		.unwrap_or("cars.txt");

	println!(
		"Loading cars from '{}' – mode: {}",
		cars_file,
		if loop_mode { "LOOP" } else { "STRAIGHT" }
	);

	let commands = match load_cars(cars_file) {
		Ok(c) if !c.is_empty() => c,
		Ok(_) => {
			eprintln!("No commands found in {}", cars_file);
			std::process::exit(1);
		}
		Err(e) => {
			eprintln!("Failed to read {}: {}", cars_file, e);
			std::process::exit(1);
		}
	};

	println!("Train has {} cars:", commands.len());
	for (i, c) in commands.iter().enumerate() {
		println!("  {:2}. {}", i + 1, c);
	}

	let engine = TrainEngine::assemble(commands, loop_mode);

	// Demo payload
	let payload = b"BINARY_CHUNK_01 hello world".to_vec();
	engine.dispatch(payload);

	// Let the train run
	thread::sleep(Duration::from_millis(500));

	// Shutdown
	engine.terminate();
	for h in engine.car_handles {
		let _ = h.join();
	}

	// Drain any remaining feedback / looped data
	while let Ok(msg) = engine.engine_rx_back.try_recv() {
		match msg {
			TrainMsg::Backward(fb) => println!("Feedback: {}", fb),
			TrainMsg::Forward(d) => {
				println!(
					"Looped data ({} bytes): {:?}",
					d.len(),
					String::from_utf8_lossy(&d)
				);
			}
			_ => {}
		}
	}

	println!("Train stopped.");
}
