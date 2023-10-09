use std::{
    fmt,
    io::{self, ErrorKind},
    ops::Deref,
};

use nvml_wrapper::{
    enum_wrappers::device::{Clock, ClockId, TemperatureSensor},
    error::NvmlError,
    Device, Nvml,
};

use crate::{errors::NvTopError, termite::LoggingHandle};

#[derive(Debug)]
pub struct GpuInfo<'d> {
    pub index: u32,
    pub inner: Device<'d>,
    pub max_memory_clock: u32,
    pub max_core_clock: u32,
    pub card_type: String,
    pub driver_version: String,
    pub cuda_version: f32,
    pub misc: String,
    pub num_cores: u32,
}

impl<'d> GpuInfo<'d> {
    pub fn from_device(index: u32, device: Device<'d>) -> Result<Self, NvmlError> {
        // Do some setup for things that will _not_ change, i.e driver version etc.
        let card_type = format!("{:?}", device.brand()?);
        let driver_version = device.nvml().sys_driver_version()?;
        let cuda_version = device.nvml().sys_cuda_driver_version()? as f32;

        let misc = format!(
            "Card: {:?}    Driver Version: {}    CUDA Version: {}",
            card_type,
            driver_version,
            cuda_version / 1000.0
        );

        Ok(GpuInfo {
            max_memory_clock: device.max_clock_info(Clock::Memory)?,
            max_core_clock: device.max_clock_info(Clock::Graphics)?,
            num_cores: device.num_cores()?,
            card_type,
            driver_version,
            cuda_version,
            misc,
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
                .for_each(|clock_type| {
                    match self.inner.clock(clock_type.clone(), clock_id.clone()) {
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

#[cfg(test)]
mod tests {
    use nvml_wrapper::{
        enum_wrappers::device::{Clock, ClockId},
        Nvml,
    };

    #[ignore = ""]
    #[test]
    fn clock_memory() {
        let nvml = Nvml::init().unwrap();
        // Get the first `Device` (GPU) in the system
        let device = nvml.device_by_index(0).unwrap();

        (0..10).for_each(|_| {
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
                    .for_each(|clock_type| {
                        match device.clock(clock_type.clone(), clock_id.clone()) {
                            Ok(value) => {
                                println!("Clock: {:?} for {:?}: {}\n", clock_type, clock_id, value);
                            }
                            Err(_err) => {}
                        }
                    });
            });
            std::thread::sleep(std::time::Duration::from_secs(1));
        });
    }
}
