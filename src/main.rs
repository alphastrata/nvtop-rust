#![allow(unused_imports, unused_variables, dead_code)]
use anyhow::Result;
use clap::Parser;
use log::{error, info, trace};
use nvml_wrapper::enum_wrappers::device::Brand;
use nvml_wrapper::enum_wrappers::device::Clock;
use nvml_wrapper::enum_wrappers::device::ClockId;
use nvml_wrapper::structs::device::UtilizationInfo;
use nvml_wrapper::Device;
use nvml_wrapper::Nvml;
use std::time;

use gpu::GpuInfo;

fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = nvtop_args::Cli::parse();

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

pub mod nvtop_args {

    use clap::Parser;

    /// Struct representing CLI arguments.
    #[derive(Parser)]
    pub(crate) struct Cli {
        /// Number of times to wait in milliseconds.
        #[clap(short, long, value_name = "MILLISECONDS")]
        delay: u32,
    }
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
