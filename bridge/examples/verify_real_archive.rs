use anyhow::{Result,ensure};
use lmu_pitwall::engineer::{analysis::{Entry,sections_for},model::*,storage::Storage};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Manifest { root:PathBuf, records:Vec<(String,u64,usize)>, entry_id:String, track:String }
fn independent_median(mut values:Vec<f64>)->Option<f64>{
    values.sort_by(f64::total_cmp);
    let count=values.len();
    if count==0 {None} else if count%2==0 {Some((values[count/2-1]+values[count/2])/2.)} else {Some(values[count/2])}
}
fn main()->Result<()> {
    let args:Vec<_>=std::env::args().collect();
    let manifest:Manifest=serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let mut entry:Option<Entry>=None;
    let(mut samples,mut snapshots,mut learned_snapshots,mut garage_snapshots,mut max_sections,mut max_clean)=(0usize,0usize,0usize,0usize,0usize,0usize);
    let mut last_check=0u64;
    let mut feasible_fuel_only_snapshots=0usize;
    let mut corner_alert_snapshots=0usize;
    let mut rules=Rules::default();
    rules.confirmed=true;
    for (path,offset,size) in &manifest.records {
        let batch=Storage::read_at(&manifest.root,path,*offset,*size)?;
        for sample in batch.samples.into_iter().filter(|s|s.track==manifest.track) {
            samples+=1;
            let stamp=sample.captured_ms;
            ensure!(sample.validate(),"Invalid real sample");
            if let Some(current)=entry.as_mut(){current.update(&batch.source_id,sample,stamp);}else{entry=Some(Entry::new(manifest.entry_id.clone(),batch.source_id.clone(),sample,stamp));}
            if stamp.saturating_sub(last_check)<1000 {continue;}
            last_check=stamp;snapshots+=1;
            let current=entry.as_ref().unwrap();
            let view=current.view(&rules,stamp);
            if view.advice.iter().any(|a|a.id.contains(":corner:")) {
                ensure!(current.last.authoritative,"Garage produced corner coaching");
                ensure!(current.laps.iter().rev().find(|l|l.valid && !l.reference_only).is_some_and(|l|l.driver==current.last.driver),"Corner coaching belongs to a former driver");
                corner_alert_snapshots+=1;
            }
            ensure!(!current.view(&rules,stamp+4000).advice.iter().any(|a|a.id.contains(":corner:")),"Stale feed produced live corner coaching");
            let clean:Vec<_>=current.laps.iter().rev().filter(|l|l.valid && !l.reference_only && l.track==current.last.track && l.vehicle==current.last.vehicle && (l.rain-current.last.rain).abs()<0.15).take(15).collect();
            max_clean=max_clean.max(clean.len());
            if !current.last.authoritative {
                garage_snapshots+=1;
                ensure!(view.strategy.scenarios.is_empty(),"Garage produced a live strategy");
                ensure!(view.strategy.range_laps.is_none(),"Garage produced a live resource range");
            }
            let fuel:Vec<_>=clean.iter().filter_map(|l|l.fuel_used_l).collect();
            if fuel.len()>=3 {
                let expected=independent_median(fuel).unwrap();
                ensure!((view.strategy.fuel_per_lap.unwrap()-expected).abs()<1e-9,"Fuel median mismatch");
                if current.last.authoritative && current.last.fuel_l.is_some() {
                    learned_snapshots+=1;
                    ensure!((view.strategy.range_laps.unwrap()-current.last.fuel_l.unwrap()/expected).abs()<1e-8,"Fuel-only range mismatch");
                    let approach=if current.last.in_pits {0.} else {(1.-current.last.lap_distance_m/current.last.track_length_m).rem_euclid(1.)};
                    let minimum=expected*(approach+rules.reserve_laps)+rules.reserve_fuel_l;
                    let full_lap=expected*(1.+rules.reserve_laps)+rules.reserve_fuel_l;
                    if current.last.fuel_l.unwrap()>minimum+0.1 && current.last.fuel_capacity_l.is_some_and(|capacity|capacity>full_lap) {
                        ensure!(view.strategy.scenarios.iter().any(|s|s.feasible),"Real LMP2 fuel-only forecast incorrectly blocked; VE={:?}",current.last.ve);
                        feasible_fuel_only_snapshots+=1;
                    }
                }
            }
            max_sections=max_sections.max(view.sections.len());
            if let Some(reference)=clean.iter().find(|l|l.samples.len()>100) {
                let zones=sections_for(reference,&rules);
                ensure!(zones.windows(2).all(|p|p[0].end_m<=p[1].start_m),"Detected braking zones overlap");
            }
        }
    }
    ensure!(samples>10000 && max_clean>=3 && learned_snapshots>10 && garage_snapshots>0 && max_sections>=3,"Incomplete real archive coverage");
    ensure!(feasible_fuel_only_snapshots>100,"Insufficient real fuel-only scenario coverage");
    let result=serde_json::json!({"samples":samples,"snapshots":snapshots,"learned_resource_snapshots":learned_snapshots,"feasible_fuel_only_snapshots":feasible_fuel_only_snapshots,"corner_alert_snapshots":corner_alert_snapshots,"stale_corner_alerts_suppressed":true,"garage_snapshots":garage_snapshots,"max_clean_laps":max_clean,"max_compared_braking_zones":max_sections,"errors":[]});
    std::fs::write(&args[2],serde_json::to_vec_pretty(&result)?)?;
    println!("{result}");
    Ok(())
}
