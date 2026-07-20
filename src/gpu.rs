use std::{
    fmt,
    io::{self, ErrorKind},
    ops::Deref,
};

use nvml_wrapper::{
    Device, Nvml,
    enum_wrappers::device::{Clock, ClockId, TemperatureSensor},
    enums::device::UsedGpuMemory,
    error::NvmlError,
};

use crate::{errors::NvTopError, termite::LoggingHandle};

#[derive(Debug, Clone)]
pub struct GpuProcess {
    pub pid: u32,
    pub used_memory: u64,
    pub name: String,
    pub is_compute: bool,
}

#[derive(Debug)]
pub struct GpuInfo<'d> {
    pub index: u32,
    pub inner: Device<'d>,
    pub max_memory_clock: u32,
    pub max_core_clock: u32,
    pub name: String,
    pub driver_version: String,
    pub cuda_version: f32,
    pub misc: String,
    pub num_cores: u32,
    pub pcie_link: String,
    pub arch: String,
}

impl<'d> GpuInfo<'d> {
    pub fn from_device(index: u32, device: Device<'d>) -> Result<Self, NvmlError> {
        let brand = format!("{:?}", device.brand()?);
        let name = device.name().unwrap_or_else(|_| brand.clone());
        let driver_version = device.nvml().sys_driver_version()?;
        let cuda_version = device.nvml().sys_cuda_driver_version()? as f32;

        let misc = format!(
            "Card: {}    Driver Version: {}    CUDA Version: {}",
            name,
            driver_version,
            cuda_version / 1000.0
        );

        // Fetch both current negotiated and max capable PCIe speeds
        let pcie_link = match (device.pcie_link_speed(), device.max_pcie_link_speed()) {
            (Ok(current), Ok(max)) => {
                let pci_gen = match current {
                    2500 => "Gen1",
                    5000 => "Gen2",
                    8000 => "Gen3",
                    16000 => "Gen4",
                    32000 => "Gen5",
                    _ => "Unknown",
                };
                let max_gen = match max.as_integer() {
                    Some(2500) | None => "N/A",
                    Some(5000) => "Gen2",
                    Some(8000) => "Gen3",
                    Some(16000) => "Gen4",
                    Some(32000) => "Gen5",
                    _ => "?",
                };
                if pci_gen != max_gen && max_gen != "N/A" && max_gen != "?" {
                    format!("⚠️ {} / {} MT/s (Max: {})", pci_gen, current, max_gen)
                } else {
                    format!("{} / {} MT/s", pci_gen, current)
                }
            }
            _ => "N/A".into(),
        };

        let arch = device.architecture().map(|a| format!("{:?}", a)).unwrap_or_default();

        Ok(GpuInfo {
            max_memory_clock: device.max_clock_info(Clock::Memory)?,
            max_core_clock: device.max_clock_info(Clock::Graphics)?,
            num_cores: device.num_cores()?,
            name,
            driver_version,
            cuda_version,
            misc,
            pcie_link,
            arch,
            index,
            inner: device,
        })
    }
}

impl<'d> Deref for GpuInfo<'d> {
    type Target = Device<'d>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl fmt::Display for GpuInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let meminfo = self.inner.memory_info().unwrap();
        let utilisation = self.inner.utilization_rates().unwrap();
        writeln!(f, "Brand: {:?}", self.inner.brand())?;
        writeln!(f, "core: {:?}%", utilisation.gpu)?;
        writeln!(f, "mem_used: {:?}", meminfo.used as f64 / 1_073_741_824.0)?;
        writeln!(f, "mem {:?}%", (meminfo.total / meminfo.used))?;
        writeln!(f, "mem_total: {:?}", meminfo.total as f64 / 1_073_741_824.0)?;
        writeln!(
            f,
            "Temp: {:?}C",
            self.inner.temperature(TemperatureSensor::Gpu)
        )?;

        [
            ClockId::Current,
            ClockId::TargetAppClock,
            ClockId::DefaultAppClock,
            ClockId::CustomerMaxBoost,
        ]
        .into_iter()
        .for_each(|clock_id| {
            [Clock::Graphics, Clock::SM, Clock::Memory, Clock::Video]
                .into_iter()
                .for_each(|clock_type| match self.inner.clock(clock_type, clock_id) {
                    Ok(value) => {
                        writeln!(f, "Clock {:?} for {:?}: {}", clock_type, clock_id, value)
                            .unwrap_or_default()
                    }
                    Err(err) => {
                        let _formatted = format!(
                            "clock_type={:?}\t\tclock_id={:?} {}",
                            clock_type, clock_id, err,
                        );
                    }
                });
        });
        Ok(())
    }
}

pub fn try_init_gpus<'n>(
    nvml: &'n Nvml,
    lh: &LoggingHandle,
) -> Result<Vec<GpuInfo<'n>>, NvTopError> {
    let count = nvml.device_count()?;
    let mut gpu_list = Vec::with_capacity(count as usize);

    for i in 0..count {
        match nvml.device_by_index(i) {
            Ok(dev) => {
                let gpu = GpuInfo::from_device(i, dev)?;
                lh.error(&format!("Compatible GPU found at [{i}]: {gpu}"));
                gpu_list.push(gpu);
            }
            Err(
                _err @ (NvmlError::InsufficientPower
                | NvmlError::NoPermission
                | NvmlError::IrqIssue
                | NvmlError::GpuLost),
            ) => {
                lh.error("Failed to init device [{i}]: {err}");
                continue; // carry on
            }
            Err(e) => return Err(e.into()),
        }
    }

    if gpu_list.is_empty() {
        Err(io::Error::new(ErrorKind::NotFound, "No compatible GPU detected").into())
    } else {
        Ok(gpu_list)
    }
}

pub fn get_gpu_processes(device: &Device) -> Result<Vec<GpuProcess>, NvmlError> {
    let mut gpu_processes = Vec::new();

    // 1. Ingest True Compute Context Processes Natively
    if let Ok(compute_processes) = device.running_compute_processes() {
        for process in compute_processes {
            let memory_bytes = match process.used_gpu_memory {
                UsedGpuMemory::Used(bytes) => bytes,
                UsedGpuMemory::Unavailable => 0,
            };

            gpu_processes.push(GpuProcess {
                pid: process.pid,
                used_memory: memory_bytes,
                is_compute: true,
                name: "".to_string(), // NVML doesn't provide names for compute processes natively
            });
        }
    }

    // 2. Ingest Graphics Context Processes Natively (NVidia Driver)
    if let Ok(graphics_processes) = device.running_graphics_processes() {
        for process in graphics_processes {
            let memory_bytes = match process.used_gpu_memory {
                UsedGpuMemory::Used(bytes) => bytes,
                UsedGpuMemory::Unavailable => 0,
            };

            gpu_processes.push(GpuProcess {
                pid: process.pid,
                used_memory: memory_bytes,
                is_compute: false,
                name: "".to_string(),
            });
        }
    }

    // 3. Ingest Process Names via Hayaku
    if gpu_processes.is_empty() {
        return Ok(gpu_processes);
    }

    // Parallel lookup of process names using the Hayaku binary pipeline stream
    use std::process::Command;

    for proc_entry in &mut gpu_processes {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(format!(
                "cat /proc/{}/cmdline 2>/dev/null | tr '\\0' ' ' | cut -d' ' -f1 | rev | cut -d'/' -f1 | rev",
                proc_entry.pid
            ))
            .stdout(std::process::Stdio::piped());

        if let Ok(output) = cmd.output() {
            if let Ok(name) = String::from_utf8(output.stdout) {
                proc_entry.name = name.trim().to_string();
            }
        } else {
            // Fallback for systems where /proc is inaccessible
            proc_entry.name = format!("pid:{}", proc_entry.pid);
        }
    }

    Ok(gpu_processes)
}
