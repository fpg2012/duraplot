use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

fn create_serialport(path: &'static str) -> Box<dyn SerialPort> {
    let port = serialport::new(path, 9600)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .open()
        .expect("open port failed");
    return port;
}

fn parse_buffer_data(buffer: &[u8; 2]) -> u16 {
    ((buffer[1] as u16) << 8) + buffer[0] as u16
}

fn parse_level_to_y(level: u16, signal_channel: i32) -> i32 {
    let level = level as i32;
    -(level / (1024 / 150) + 25) + (signal_channel + 1) * 200
}

fn parse_time_to_x(time: i32) -> i32 {
    time * 2
}

fn main() {
    // the port that arduino uno uses on Linux
    let tty_path = "/dev/ttyACM0";

    let line_color = [
        Color::RGB(140, 180, 140),
        Color::RGB(180, 140, 140),
        Color::RGB(140, 140, 180),
        Color::RGB(180, 180, 140),
    ];
    let bg_color = Color::RGB(0, 0, 0);

    // channel for sending data
    let (tx_data, rx_data) = std::sync::mpsc::channel();
    // channel for controlling producer
    let (tx_ready, rx_ready) = std::sync::mpsc::channel();

    // producer, read data from serial port and parse it
    let producer = std::thread::spawn(move || {
        let mut buffer: [u8; 2] = [0, 0];
        rx_ready.recv().unwrap(); // wait for consumer ready
        let mut port = create_serialport(tty_path);
        loop {
            match rx_ready.try_recv() {
                Ok(_) => break, // consumer exited, stop send data
                _ => (),
            }
            match port.read_exact(&mut buffer) {
                Ok(_) => (),
                Err(_) => continue, // ignore error, read again
            }
            // concat the twe u8 into a u16
            let data = parse_buffer_data(&buffer);
            // send it, block until consumer receive it
            tx_data.send(data).unwrap();
        }
    });

    // consumer, receive the voltage level and draw it in window
    let consumer = std::thread::spawn(move || {
        let sdl = sdl2::init().unwrap();
        let video_subsystem = sdl.video().unwrap();
        // enable antialiasing
        // video_subsystem.gl_attr().set_multisample_buffers(1);
        // video_subsystem.gl_attr().set_multisample_samples(2);
        let window = video_subsystem
            .window("Duraplot", 1500, 800)
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().build().unwrap();
        let mut event_pump = sdl.event_pump().unwrap();

        // the window is divided into 4 channels from top to the bottom
        // variable `channel` indicates the current channel
        // should between 0 and 3
        let mut channel = 0;

        // clear window with background color
        canvas.set_draw_color(bg_color);
        canvas.clear();
        // switch color to color of channel-0
        canvas.set_draw_color(line_color[channel]);
        // consumer now ready to receive data, tell producer to start itself
        tx_ready.send(()).unwrap();

        let mut time: i32 = 0;
        let mut last_point = (0, 0);

        // `detached` indicates whether consumer is detach from producer, that is, ignore all the incoming data
        let mut detached = false;

        // render loop
        'main: loop {
            if !detached {
                // not detached, draw new line in the window
                match rx_data.try_recv() {
                    Ok(num) => {
                        // println!("{}, Read data {}", time, num);
                        let y = parse_level_to_y(num, channel as i32);
                        let x = parse_time_to_x(time);
                        canvas
                            .draw_line(last_point, (x, y))
                            .expect("draw line failed");
                        canvas.present();
                        time += 1;
                        last_point = (x, y);
                    }
                    Err(_) => (),
                }
            } else {
                // ignore all data
                match rx_data.try_recv() {
                    Ok(_) => (),  // dispose value
                    Err(_) => (), // ignore error
                }
            }
            for event in event_pump.poll_event() {
                match event {
                    Event::Quit { .. } => {
                        tx_ready.send(()).unwrap();
                        break 'main;
                    }
                    Event::KeyDown { keycode, .. } => match keycode.unwrap() {
                        Keycode::S => {
                            detached = true; // stop, or detach
                        }
                        Keycode::R => detached = false,
                        Keycode::N => {
                            // switch to next channel
                            channel = (channel + 1) % 4;
                            canvas.set_draw_color(line_color[channel]);
                            time = 0;
                            last_point = (0, 0);
                        }
                        Keycode::C => {
                            // clear the current channel
                            time = 0;
                            last_point = (0, 0);
                            let backup = canvas.draw_color();
                            canvas.set_draw_color(bg_color);
                            canvas
                                .fill_rect(Rect::new(0, (channel as i32) * 200, 1500, 200))
                                .expect("failed to draw rect");
                            canvas.present();
                            canvas.set_draw_color(backup);
                        }
                        _ => (), // ignore other keystrokes
                    },
                    _ => (), // ignore other events
                }
            }
        }
    });
    consumer.join().unwrap();
    producer.join().unwrap();
}
