use std::{fmt, ops::Deref};

use nvml_wrapper::{
    enum_wrappers::device::{Clock, ClockId, TemperatureSensor},
    Device,
};

#[derive(Debug)]
pub struct GpuInfo<'d> {
    pub inner: &'d Device<'d>,
    pub max_memory_clock: u32,
    pub max_core_clock: u32,
    pub card_type: String,
    pub driver_version: String,
    pub cuda_version: f32,
    pub misc: String,
}

impl<'d> Deref for GpuInfo<'d> {
    type Target = &'d Device<'d>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'d> fmt::Display for GpuInfo<'d> {
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
                            write!(f, "Clock {:?} for {:?}: {}\n", clock_type, clock_id, value)
                                .unwrap_or_default()
                        }
                        Err(_err) => {
                            log::error!("{_err}")
                        }
                    }
                });
        });
        Ok(())
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

        (0..10).into_iter().for_each(|_| {
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
                                println!("Clock {:?} for {:?}: {}\n", clock_type, clock_id, value);
                            }
                            Err(_err) => {}
                        }
                    });
            });
            std::thread::sleep(std::time::Duration::from_secs(1));
        });
    }
}
