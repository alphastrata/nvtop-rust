use std::{fmt, ops::Deref};

use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, Device};

#[derive(Debug)]
pub struct GpuInfo<'d> {
    pub inner: &'d Device<'d>,
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
        write!(f, "Brand: {:?}\n", self.inner.brand())?;
        write!(f, "core: {:?}%\n", utilisation.gpu)?;
        write!(f, "mem_used: {:?}\n", meminfo.used as f64 / 1_073_741_824.0)?;
        write!(f, "mem {:?}%\n", (meminfo.total / meminfo.used))?;
        write!(
            f,
            "mem_total: {:?}\n",
            meminfo.total as f64 / 1_073_741_824.0
        )?;
        write!(
            f,
            "Temp: {:?}C\n",
            self.inner.temperature(TemperatureSensor::Gpu)
        )?;

        // TODO: the other stuff we may want to print...

        // for clock_id in [
        //     ClockId::Current,
        //     ClockId::TargetAppClock,
        //     ClockId::DefaultAppClock,
        //     ClockId::CustomerMaxBoost,
        // ]
        // .into_iter()
        // {
        //     [Clock::Graphics, Clock::SM, Clock::Memory, Clock::Video]
        //         .into_iter()
        //         .for_each(|clock_type| {
        //             match self.device.clock(clock_type.clone(), clock_id.clone()) {
        //                 Ok(value) => {
        //                     write!(f, "Clock {:?} for {:?}: {}\n", clock_type, clock_id, value)
        //                         .unwrap_or_default()
        //                 }
        //                 Err(_err) => {
        //                     log::error!("{_err}")
        //                 }
        //             }
        //         });
        // }
        Ok(())
    }
}
