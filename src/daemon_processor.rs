use std::time::{Duration, SystemTime, UNIX_EPOCH};
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use serde::Serialize;
use crate::{gpu::get_gpu_processes, termite::LoggingHandle, errors::NvTopError};

// Ingesting the high-performance binary transport channel primitives from our hayaku dependency
use hayaku::sender::HayakuSender;

#[derive(Serialize)]
struct TelemetryExportPacket {
    timestamp_ms: u128,
    telemetry_tick: bool,
    gpu_utilization_pct: u32,
    memory_utilization_pct: u32,
    sm_clock_mhz: u32,
    graphics_clock_mhz: u32,
    memory_clock_mhz: u32,
    temperature_c: u32,
    power_usage_w: f32,
    vram_total_bytes: u64,
    vram_used_bytes: u64,
    hook_pid_active: bool,
    target_pid_allocated_bytes: u64,
}

pub fn execute_streaming_daemon(
    nvml: &Nvml,
    interval: Duration,
    output_target: &str,
    target_pid: Option<u32>,
    lh: &LoggingHandle,
) -> Result<(), NvTopError> {
    let device = nvml.device_by_index(0)?;
    
    // Bind hayaku binary framing layer over the target pipeline stream file destination
    let mut hayaku_sender = HayakuSender::open(output_target)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    lh.info("Streaming daemon linked to hayaku pipeline channel.");

    loop {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        
        let (util_gpu, util_mem) = device.utilization_rates()
            .map(|u| (u.gpu, u.memory))
            .unwrap_or((0, 0));
        let (mem_total, mem_used) = device.memory_info()
            .map(|m| (m.total, m.used))
            .unwrap_or((0, 0));
        
        let sm_clk = device.clock(Clock::SM, nvml_wrapper::enum_wrappers::device::ClockId::Current).unwrap_or(0);
        let gr_clk = device.clock(Clock::Graphics, nvml_wrapper::enum_wrappers::device::ClockId::Current).unwrap_or(0);
        let mem_clk = device.clock(Clock::Memory, nvml_wrapper::enum_wrappers::device::ClockId::Current).unwrap_or(0);
        let temp = device.temperature(TemperatureSensor::Gpu).unwrap_or(0);
        let power_mw = device.power_usage().unwrap_or(0);

        // Track precise state flags over the specific PID targeting hook
        let mut target_allocated_memory = 0;
        let mut pid_found = false;

        if let Some(pid) = target_pid {
            if let Ok(active_processes) = get_gpu_processes(&device) {
                if let Some(target_proc) = active_processes.into_iter().find(|p| p.pid == pid) {
                    target_allocated_memory = target_proc.used_memory;
                    pid_found = true;
                }
            }
        }

        let packet = TelemetryExportPacket {
            timestamp_ms: now,
            telemetry_tick: true,
            gpu_utilization_pct: util_gpu,
            memory_utilization_pct: util_mem,
            sm_clock_mhz: sm_clk,
            graphics_clock_mhz: gr_clk,
            memory_clock_mhz: mem_clk,
            temperature_c: temp,
            power_usage_w: (power_mw as f32) / 1000.0,
            vram_total_bytes: mem_total,
            vram_used_bytes: mem_used,
            hook_pid_active: pid_found,
            target_pid_allocated_bytes: target_allocated_memory,
        };

        if let Ok(serialized_payload) = serde_json::to_vec(&packet) {
            // Write structured frame data using hayaku binary encapsulation wrappers
            if let Err(err) = hayaku_sender.write_frame(&serialized_payload) {
                lh.error(&format!("Hayaku channel pipe streaming failure: {err}"));
            }
        }

        std::thread::sleep(interval);
    }
}
