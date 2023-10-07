#![allow(unused_imports, unused_variables, dead_code)]
use anyhow::Result;

use nvml_wrapper::enum_wrappers::device::Brand;
use nvml_wrapper::enum_wrappers::device::Clock;
use nvml_wrapper::enum_wrappers::device::ClockId;
use nvml_wrapper::structs::device::UtilizationInfo;
use nvml_wrapper::Device;

use nvml_wrapper::Nvml;

fn main() -> Result<()> {
    let nvml = Nvml::init()?;
    // Get the first `Device` (GPU) in the system
    let device = nvml.device_by_index(0)?;

    let brand = device.brand()?; // GeForce on my system
    let fan_speed = device.fan_speed(0)?; // Currently 17% on my system
    let power_limit = device.enforced_power_limit()?; // 275k milliwatts on my system
    let encoder_util = device.encoder_utilization()?; // Currently 0 on my system; Not encoding anything
    let memory_info = device.memory_info()?; // Currently 1.63/6.37 GB used on my system

    Ok(())
}

/// Struct representing GPU information.
pub struct GpuInfo<'d> {
    pub device: &'d Device<'d>,
    pub brand: Brand,
    pub fan_speed_percent: u32,
    pub power_limit_milliwatts: u32,
    pub encoder_utilization_percent: UtilizationInfo,
    pub memory_used_gb: f64,
    pub clock_graphics_current: u32,
    pub clock_sm_current: u32,
    pub clock_memory_current: u32,
    pub clock_video_current: u32,
    pub clock_graphics_target_app: u32,
    pub clock_sm_target_app: u32,
    pub clock_memory_target_app: u32,
    pub clock_graphics_default_app: u32,
    pub clock_sm_default_app: u32,
    pub clock_memory_default_app: u32,
}

impl<'d> GpuInfo<'d> {
    pub fn new<D>(device: &'d nvml_wrapper::Device) -> Result<GpuInfo<'d>> {
        let fan_speed = device.fan_speed(0)?;
        let power_limit = device.enforced_power_limit()?;
        let encoder_util = device.encoder_utilization()?;
        let memory_info = device.memory_info()?;
        let clock_graphics_current = device.clock(Clock::Graphics, ClockId::Current)?;
        let clock_sm_current = device.clock(Clock::SM, ClockId::Current)?;
        let clock_memory_current = device.clock(Clock::Memory, ClockId::Current)?;
        let clock_video_current = device.clock(Clock::Video, ClockId::Current)?;
        let clock_graphics_target_app = device.clock(Clock::Graphics, ClockId::TargetAppClock)?;
        let clock_sm_target_app = device.clock(Clock::SM, ClockId::TargetAppClock)?;
        let clock_memory_target_app = device.clock(Clock::Memory, ClockId::TargetAppClock)?;
        let clock_graphics_default_app = device.clock(Clock::Graphics, ClockId::DefaultAppClock)?;
        let clock_sm_default_app = device.clock(Clock::SM, ClockId::DefaultAppClock)?;
        let clock_memory_default_app = device.clock(Clock::Memory, ClockId::DefaultAppClock)?;

        Ok(GpuInfo {
            brand: device.brand()?,
            memory_used_gb: memory_info.used as f64 / 1_073_741_824.0, // Convert to GB
            clock_graphics_current,
            clock_sm_current,
            clock_memory_current,
            clock_video_current,
            clock_graphics_target_app,
            clock_sm_target_app,
            clock_memory_target_app,
            clock_graphics_default_app,
            clock_sm_default_app,
            clock_memory_default_app,
            fan_speed_percent: fan_speed,
            power_limit_milliwatts: power_limit,
            encoder_utilization_percent: encoder_util,
            device,
        })
    }
}

use std::fmt;

impl<'d> fmt::Display for GpuInfo<'d> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Brand: {:?}\n", self.brand)?;
        write!(f, "Fan Speed: {:?}%\n", self.fan_speed_percent)?;
        write!(
            f,
            "Power Limit: {:?} milliwatts\n",
            self.power_limit_milliwatts
        )?;
        write!(
            f,
            "Encoder Utilization: {:?}\n",
            self.encoder_utilization_percent
        )?;
        write!(
            f,
            "Memory Info: {:.2} GB used out of {:.2} GB\n",
            self.memory_used_gb, self.memory_used_gb
        )?;

        // TODO: the other stuff we may want to print...

        Ok(())
    }
}
