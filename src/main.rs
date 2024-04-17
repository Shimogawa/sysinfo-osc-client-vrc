use std::{
    cell::RefCell,
    io,
    net::UdpSocket,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
    vec,
};

use clap::Parser;
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, Device, Nvml};
use once_cell::sync::Lazy;
use rosc::{OscMessage, OscPacket, OscType};
use sysinfo::System;

static NVML_INSTANCE: Lazy<Nvml> = Lazy::new(|| Nvml::init().unwrap());

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Do not show time
    #[arg(short = 't', long)]
    no_time: bool,

    /// Do not show cpu usage
    #[arg(short = 'c', long)]
    no_cpu: bool,

    /// Do not show ram usage
    #[arg(short = 'r', long)]
    no_ram: bool,

    /// Do not show gpu usage
    #[arg(short = 'g', long)]
    no_gpu: bool,

    /// Time interval in seconds
    #[arg(short, long, default_value_t = 3, value_parser = clap::value_parser!(u64).range(1..))]
    interval: u64,
}

trait Info {
    fn get_info(&mut self) -> String;
}

struct TimeInfo;

impl Info for TimeInfo {
    fn get_info(&mut self) -> String {
        format!(
            "{}",
            chrono::Local::now().format("%m/%d/%Y %H:%M:%S UTC%:::z")
        )
    }
}

struct CpuInfo {
    sys: Rc<RefCell<System>>,
}

impl CpuInfo {
    pub fn new(sys: Rc<RefCell<System>>) -> Self {
        Self { sys }
    }
}

impl Info for CpuInfo {
    fn get_info(&mut self) -> String {
        self.sys.borrow_mut().refresh_cpu();
        self.sys.borrow_mut().refresh_processes();
        format!(
            "CPU: {:.2}%, Processes: {:?}",
            self.sys.borrow().global_cpu_info().cpu_usage(),
            self.sys.borrow().processes().len(),
        )
    }
}

struct RamInfo {
    sys: Rc<RefCell<System>>,
}

impl RamInfo {
    pub fn new(sys: Rc<RefCell<System>>) -> Self {
        Self { sys }
    }
}

impl Info for RamInfo {
    fn get_info(&mut self) -> String {
        self.sys.borrow_mut().refresh_memory();
        format!(
            "RAM: {} ({:.2}%)",
            bytesize::to_string(self.sys.borrow().used_memory(), true),
            self.sys.borrow().used_memory() as f32 / self.sys.borrow().total_memory() as f32
                * 100.0
        )
    }
}

struct GpuInfo<'a> {
    device: Box<Device<'a>>,
}

impl<'a> GpuInfo<'a> {
    pub fn new() -> Self {
        Self {
            device: Box::new(NVML_INSTANCE.device_by_index(0).unwrap()),
        }
    }
}

impl<'a> Info for GpuInfo<'a> {
    fn get_info(&mut self) -> String {
        let mem_info = self.device.memory_info().unwrap();
        format!(
            "GPU: {}% ({:.2}W{})\n{} ({:.2}%)",
            self.device.utilization_rates().unwrap().gpu,
            self.device.power_usage().unwrap() as f32 / 1000.0,
            match self.device.temperature(TemperatureSensor::Gpu) {
                Ok(temp) => format!(", {}°C", temp),
                Err(_) => "".to_string(),
            },
            bytesize::to_string(mem_info.used, true),
            mem_info.used as f32 / mem_info.total as f32 * 100.0,
        )
    }
}

fn main() -> io::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let args = Args::parse();

    {
        let socket = UdpSocket::bind("127.0.0.1:9001")?;
        let mut buf = Vec::new();
        let sys = Rc::new(RefCell::new(System::new_all()));

        let mut infos: Vec<Box<dyn Info>> = Vec::new();
        if !args.no_time {
            infos.push(Box::new(TimeInfo));
        }
        if !args.no_cpu {
            infos.push(Box::new(CpuInfo::new(Rc::clone(&sys))));
        }
        if !args.no_ram {
            infos.push(Box::new(RamInfo::new(Rc::clone(&sys))));
        }
        if !args.no_gpu {
            infos.push(Box::new(GpuInfo::new()));
        }

        while running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(args.interval));
            let info = get_info(&mut infos);
            if info.is_empty() {
                continue;
            }
            let msg = OscMessage {
                addr: "/chatbox/input".to_string(),
                args: vec![rosc::OscType::String(info.clone()), OscType::Bool(true)],
            };
            let packet = OscPacket::Message(msg);
            rosc::encoder::encode_into(&packet, &mut buf).unwrap();

            socket.send_to(&buf, "127.0.0.1:9000")?;
            println!("Sent: {:?}", info);

            unsafe {
                buf.set_len(0);
            }
        }
    } // the socket is closed here
    println!("bye");
    Ok(())
}

fn get_info(providers: &mut Vec<Box<dyn Info>>) -> String {
    let mut info_str = String::new();

    for provider in providers {
        info_str.push_str(provider.get_info().as_str());
        info_str.push_str("\n");
    }

    info_str.pop();
    info_str
}
