use std::fs::OpenOptions;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, ClockId, PcieUtilCounter, TemperatureSensor};
use serde::Serialize;

use crate::errors::NvTopError;
use crate::gpu::get_gpu_processes;
use crate::hayaku_server::spawn as hayaku_spawn;
use crate::termite::LoggingHandle;

#[derive(Serialize)]
pub struct TelemetryExportPacket {
    pub timestamp_ms: u128,
    pub gpu_utilization_pct: u32,
    pub memory_utilization_pct: u32,
    pub sm_clock_mhz: u32,
    pub graphics_clock_mhz: u32,
    pub memory_clock_mhz: u32,
    pub temperature_c: u32,
    pub power_usage_w: f32,
    pub vram_total_bytes: u64,
    pub vram_used_bytes: u64,
    pub pcie_current_speed_mbps: u32,
    pub pcie_max_speed_enum: String,
    pub pcie_throughput_tx_bytes: u64,
    pub pcie_throughput_rx_bytes: u64,
    pub bus_type: String,
    pub device_architecture: String,
    pub power_source: String,
    pub fw_version: String,
    pub fan_control_policy: Option<String>,
    pub gpm_gpu_util_percent: f32,
    pub gpm_sm_util_percent: f32,
    pub gpc_tx_per_sec_mbps: f32,
    pub gpc_rx_per_sec_mbps: f32,
}

fn is_tcp_mode(output_path: &str) -> bool {
    output_path.starts_with("tcp://") || output_path.contains(':')
}

pub fn execute_streaming_daemon(
    nvml: &Nvml,
    interval: Duration,
    output_path: &str,
    target_pid: Option<u32>,
    _lh: &LoggingHandle,
) -> Result<(), NvTopError> {
    let device = &nvml.device_by_index(0)?;

    let bus_type = device
        .bus_type()
        .map(|b| format!("{b:?}"))
        .unwrap_or_else(|_| "Unknown".into());
    let arch = device
        .architecture()
        .map(|a| format!("{a:?}"))
        .unwrap_or_else(|_| "Unknown".into());
    let power_src = device
        .power_source()
        .map(|p| format!("{p:?}"))
        .unwrap_or_else(|_| "Unknown".into());
    let fw_ver = device
        .gsp_firmware_version()
        .unwrap_or_else(|_| "N/A".into());
    let fan_policy = device.fan_control_policy(0).ok().map(|p| format!("{p:?}"));

    if is_tcp_mode(output_path) {
        let addr = output_path.trim_start_matches("tcp://");
        eprintln!("[DAEMON] TCP socket mode: {addr}");

        let (_, tx) = hayaku_spawn(addr);

        loop {
            let packet = build_packet(
                device,
                &bus_type,
                &arch,
                &power_src,
                &fw_ver,
                &fan_policy,
                target_pid,
            )?;
            let json_line = match serde_json::to_string(&packet) {
                Ok(s) => s + "\n",
                Err(e) => {
                    eprintln!("[DAEMON] Serde error: {e}");

                    continue;
                }
            };

            if tx.send(json_line.into_bytes()).is_err() {
                eprintln!("[DAEMON] Broadcast channel failed.");
                break;
            }

            std::thread::sleep(interval);
        }
    } else {
        eprintln!("[DAEMON] Starting local file stream to: {output_path}");

        let mut fh = OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_path)
            .map_err(|e| io::Error::other(e))?;

        loop {
            let packet = build_packet(
                device,
                &bus_type,
                &arch,
                &power_src,
                &fw_ver,
                &fan_policy,
                target_pid,
            )?;
            let json_line = serde_json::to_string(&packet)
                .map_err(|e| io::Error::other(e))?
                + "\n";

            fh.write_all(json_line.as_bytes())?;
            fh.flush()?;

            std::thread::sleep(interval);
        }
    }

    Ok(())
}

fn build_packet(
    device: &nvml_wrapper::Device<'_>,
    bus_type: &str,
    arch: &str,
    power_src: &str,
    fw_ver: &str,
    fan_policy: &Option<String>,
    target_pid: Option<u32>,
) -> Result<TelemetryExportPacket, io::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let (util_gpu, util_mem) = device
        .utilization_rates()
        .map(|u| (u.gpu, u.memory))
        .unwrap_or((0, 0));
    let (mem_total, mem_used) = device
        .memory_info()
        .map(|m| (m.total, m.used))
        .unwrap_or((0, 0));
    let sm_clk = device.clock(Clock::SM, ClockId::Current).unwrap_or(0);
    let gr_clk = device.clock(Clock::Graphics, ClockId::Current).unwrap_or(0);
    let mem_clk = device.clock(Clock::Memory, ClockId::Current).unwrap_or(0);
    let temp = device.temperature(TemperatureSensor::Gpu).unwrap_or(0);
    let power_mw = device.power_usage().unwrap_or(0);

    let (pcie_current_mbps, pcie_max_enum) =
        match (device.pcie_link_speed(), device.max_pcie_link_speed()) {
            (Ok(cur), Ok(max)) => {
                if cur == 0 || max.as_integer().unwrap_or(0) == 0 {
                    (0, "N/A".to_string())
                } else {
                    (cur, format!("{max:?}"))
                }
            }
            _ => (0, "Unavailable".into()),
        };

    let (tx_bytes, rx_bytes) = match (
        device.pcie_throughput(PcieUtilCounter::Send),
        device.pcie_throughput(PcieUtilCounter::Receive),
    ) {
        (Ok(tx), Ok(rx)) => (
            (tx as u64).saturating_mul(1024),
            (rx as u64).saturating_mul(1024),
        ),
        _ => (0, 0),
    };

    if let Some(pid) = target_pid {
        get_gpu_processes(device)
            .ok()
            .and_then(|procs| {
                procs
                    .into_iter()
                    .find(|p| p.pid == pid)
                    .map(|p| p.used_memory)
            })
            .unwrap_or(0)
    } else {
        0
    };

    Ok(TelemetryExportPacket {
        timestamp_ms: now,
        gpu_utilization_pct: util_gpu,
        memory_utilization_pct: util_mem,
        sm_clock_mhz: sm_clk,
        graphics_clock_mhz: gr_clk,
        memory_clock_mhz: mem_clk,
        temperature_c: temp,
        power_usage_w: (power_mw as f32) / 1000.0,
        vram_total_bytes: mem_total,
        vram_used_bytes: mem_used,
        pcie_current_speed_mbps: pcie_current_mbps,
        pcie_max_speed_enum: pcie_max_enum,
        pcie_throughput_tx_bytes: tx_bytes,
        pcie_throughput_rx_bytes: rx_bytes,
        bus_type: bus_type.to_string(),
        device_architecture: arch.to_string(),
        power_source: power_src.to_string(),
        fw_version: fw_ver.to_string(),
        fan_control_policy: fan_policy.clone(),
        gpm_gpu_util_percent: 0.0,
        gpm_sm_util_percent: 0.0,
        gpc_tx_per_sec_mbps: 0.0,
        gpc_rx_per_sec_mbps: 0.0,
    })
}
