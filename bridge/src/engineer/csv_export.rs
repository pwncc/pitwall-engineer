//! Scalar telemetry export. Preserve the initial columns and append context and health.
use super::model::{Batch, Sample};
use std::fmt::Write;

fn cell(text: &str) -> String {
    let prefix = if text.trim_start().starts_with(['=', '+', '-', '@']) {
        "'"
    } else {
        ""
    };
    format!("\"{prefix}{}\"", text.replace('"', "\"\""))
}

pub(super) fn header() -> String {
    let mut fields: Vec<String> = concat!(
        "captured_ms,game_s,driver,lap,distance_m,speed_kmh,throttle,brake,steering,fuel_l,ve,x,z,",
        "event_id,entry_id,source_id,run_id,track,vehicle,class,session_kind,phase,slot,authoritative,valid_lap,in_pits,pit_state,yellow,",
        "track_length_m,lap_start_s,last_lap_s,session_remaining_s,fuel_capacity_l,soc,water_c,oil_c,rpm,gear,clutch,accel_x_m_s2,accel_y_m_s2,accel_z_m_s2,",
        "tc,abs,tc_active,abs_active,brake_bias_front,motor_map,migration,regen_kw,track_limits,penalties,rain,wetness,air_c,track_c,wind_x_m_s,wind_y_m_s,wind_z_m_s,impact,impact_s,detached"
    ).split(',').map(str::to_owned).collect();
    for zone in 1..=8 {
        fields.push(format!("damage_zone_{zone}"));
    }
    for corner in ["fl", "fr", "rl", "rr"] {
        for field in [
            "temperature_left_c",
            "temperature_center_c",
            "temperature_right_c",
            "carcass_c",
            "pressure_kpa",
            "remaining",
            "brake_c",
            "compound",
            "detached",
            "suspension_m",
            "ride_height_m",
            "rotation_rad_s",
        ] {
            fields.push(format!("{corner}_{field}"));
        }
    }
    fields.join(",") + "\r\n"
}

pub(super) fn row(out: &mut String, batch: &Batch, s: &Sample) -> std::fmt::Result {
    let legacy = s
        .supplemental
        .as_ref()
        .and_then(|v| v.get("adapter"))
        .and_then(|v| v.as_str())
        == Some("rF2 SMMP 3.7");
    let native_electronics = s.authoritative && !legacy;
    macro_rules! n {
        ($value:expr) => {
            write!(out, ",{}", $value)?
        };
    }
    macro_rules! o {
        ($value:expr) => {{
            out.push(',');
            if let Some(value) = $value {
                write!(out, "{value}")?;
            }
        }};
    }
    macro_rules! t {
        ($value:expr) => {{
            out.push(',');
            out.push_str(&cell($value));
        }};
    }
    write!(
        out,
        "{},{},{},{},{},{},{},{},{}",
        s.captured_ms,
        s.game_s,
        cell(&s.driver),
        s.lap,
        s.lap_distance_m,
        s.speed_kmh,
        s.throttle,
        s.brake,
        s.steering
    )?;
    o!(s.fuel_l);
    o!(s.ve);
    n!(s.x);
    n!(s.z);
    t!(&batch.event_id);
    t!(&batch.entry_id);
    t!(&batch.source_id);
    t!(&batch.run_id);
    t!(&s.track);
    t!(&s.vehicle);
    t!(&s.class);
    n!(s.session_kind);
    n!(s.phase);
    o!(s.slot);
    n!(s.authoritative);
    n!(s.valid_lap);
    n!(s.in_pits);
    n!(s.pit_state);
    n!(s.yellow);
    n!(s.track_length_m);
    n!(s.lap_start_s);
    o!(s.last_lap_s);
    o!(s.session_remaining_s);
    o!(s.fuel_capacity_l);
    o!((!legacy).then_some(s.soc).flatten());
    o!(s.water_c);
    o!(s.oil_c);
    n!(s.rpm);
    n!(s.gear);
    n!(s.clutch);
    for value in s.accel {
        n!(value);
    }
    o!(native_electronics.then_some(s.tc));
    o!(native_electronics.then_some(s.abs));
    o!(native_electronics.then_some(s.tc_active));
    o!(native_electronics.then_some(s.abs_active));
    o!(s.authoritative.then_some(s.brake_bias_front));
    o!(native_electronics.then_some(s.motor_map));
    o!(native_electronics.then_some(s.migration));
    o!(native_electronics.then_some(s.regen_kw));
    o!(native_electronics.then_some(s.track_limits));
    n!(s.penalties);
    n!(s.rain);
    n!(s.wetness);
    n!(s.air_c);
    n!(s.track_c);
    for value in s.wind {
        // This legacy reader does not acquire the scoring wind vector.
        o!((!legacy).then_some(value));
    }
    n!(s.impact);
    n!(s.impact_s);
    n!(s.detached);
    for value in s.dents {
        n!(value);
    }
    for wheel in &s.wheels {
        for index in 0..3 {
            o!(wheel.temperature_c.map(|values| values[index]));
        }
        o!(wheel.carcass_c);
        o!(wheel.pressure_kpa);
        o!(wheel.remaining);
        o!(wheel.brake_c);
        n!(wheel.compound);
        n!(wheel.detached);
        n!(wheel.suspension_m);
        n!(wheel.ride_height_m);
        n!(wheel.rotation);
    }
    out.push_str("\r\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_export_preserves_supported_readings_without_inventing_electronics() {
        let batch = Batch {
            version: 1,
            event_id: "event".into(),
            entry_id: "entry".into(),
            source_id: "pc".into(),
            run_id: "run".into(),
            seq: 1,
            historical: false,
            samples: vec![],
        };
        let mut sample = Sample {
            authoritative: true,
            fuel_l: Some(70.),
            soc: Some(0.5),
            brake_bias_front: 0.4875,
            wind: [1., 2., 3.],
            ..Default::default()
        };
        sample.wheels[0].pressure_kpa = Some(160.);
        let columns = header();
        let columns: Vec<_> = columns.trim().split(',').collect();
        let value = |s: &Sample, field: &str| {
            let mut output = String::new();
            row(&mut output, &batch, s).unwrap();
            let cells: Vec<_> = output.trim().split(',').collect();
            assert_eq!(cells.len(), columns.len());
            cells[columns.iter().position(|name| *name == field).unwrap()].to_owned()
        };
        assert_eq!(
            value(&sample, "tc"),
            "0",
            "A supported zero is a real reading"
        );
        assert_eq!(value(&sample, "tc_active"), "false");
        assert_eq!(value(&sample, "soc"), "0.5");
        sample.supplemental = Some(serde_json::json!({"adapter":"rF2 SMMP 3.7"}));
        for field in [
            "tc",
            "abs",
            "tc_active",
            "abs_active",
            "motor_map",
            "migration",
            "regen_kw",
            "track_limits",
            "soc",
            "wind_x_m_s",
            "wind_y_m_s",
            "wind_z_m_s",
        ] {
            assert_eq!(
                value(&sample, field),
                "",
                "Unsupported {field} appeared to be measured"
            );
        }
        assert_eq!(value(&sample, "fuel_l"), "70");
        assert_eq!(value(&sample, "brake_bias_front"), "0.4875");
        assert_eq!(value(&sample, "fl_pressure_kpa"), "160");
        sample.supplemental = None;
        sample.authoritative = false;
        assert_eq!(value(&sample, "tc"), "");
        assert_eq!(value(&sample, "brake_bias_front"), "");
        assert_eq!(
            value(&sample, "wind_x_m_s"),
            "1",
            "Scoring weather remains available without a driving seat"
        );
    }
}
