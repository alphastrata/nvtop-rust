#![allow(unused_imports, unused_variables, dead_code)]
use std::time;

use anyhow::Result;
use log::{error, info, trace};

use gpu::GpuInfo;
use nvml_wrapper::enum_wrappers::device::Brand;
use nvml_wrapper::enum_wrappers::device::Clock;
use nvml_wrapper::enum_wrappers::device::ClockId;
use nvml_wrapper::structs::device::UtilizationInfo;
use nvml_wrapper::Device;

use nvml_wrapper::Nvml;

fn main() -> Result<()> {
    pretty_env_logger::init();

    let nvml = Nvml::init()?;
    // Get the first `Device` (GPU) in the system
    let device = nvml.device_by_index(0)?;

    let gpu: GpuInfo = GpuInfo { inner: &device };

    let t1 = std::time::Instant::now();
    loop {
        // println!("{:#?}", gpu.device.utilization_rates().unwrap());
        println!("GpuInfo:\n{}", gpu);

        if t1.elapsed().as_secs() > 10 {
            break;
        }
    }

    Ok(())
}

pub mod gpu {

    use std::{fmt, ops::Deref};

    use nvml_wrapper::Device;

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
            write!(f, "Brand: {:?}\n", self.inner.brand())?;

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
}
