"""Exercise the actual portable receiver and embedded-configuration driver EXEs.

Uses isolated synthetic data under test_data. No game controls or network settings.
"""
import argparse
import csv as csv_module
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import sqlite3
import struct
import subprocess
import time
import urllib.parse
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--http-port", type=int, default=20437)
    parser.add_argument("--ingest-port", type=int, default=20443)
    parser.add_argument("--endpoint-host", default="127.0.0.1")
    parser.add_argument("--native-recording")
    parser.add_argument("--agents",type=int,default=3)
    parser.add_argument("--seconds",type=int,default=450)
    parser.add_argument("--speed",type=float,default=20)
    args = parser.parse_args()
    assert 1 <= args.agents <= 16 and args.seconds >= 450 and 1 <= args.speed <= 20
    bundle = args.bundle.resolve(strict=True)
    repo = Path(__file__).resolve().parent.parent
    root = repo / "test_data" / ("release-check-" + datetime.datetime.now().strftime("%Y%m%d-%H%M%S"))
    root.mkdir(parents=True)
    for line in (bundle / "SHA256SUMS.txt").read_text(encoding="utf-8-sig").splitlines():
        digest, name = line.split("  ", 1)
        target = (bundle / name).resolve(strict=True)
        assert target.is_relative_to(bundle)
        assert hashlib.sha256(target.read_bytes()).hexdigest() == digest, name

    url = f"http://127.0.0.1:{args.http_port}"
    def request(path, data=None):
        req = urllib.request.Request(url + path, data=None if data is None else json.dumps(data).encode(),
                                     headers={} if data is None else {"Content-Type": "application/json"})
        return urllib.request.urlopen(req, timeout=20).read()
    def state():
        return json.loads(request("/api/state"))
    def launch(exe, *options):
        return subprocess.Popen([str(exe), *map(str, options)], creationflags=subprocess.CREATE_NO_WINDOW)
    def receiver():
        process = launch(bundle / "lmu-engineer.exe", "--data", root / "receiver", "--http-port", args.http_port,
                         "--ingest-port", args.ingest_port, "--no-browser", "--no-tray")
        # A complete eight-car archive takes longer to reconstruct than a fresh profile.
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            try:
                state()
                return process
            except Exception:
                assert process.poll() is None, "Receiver exited before becoming ready"
                time.sleep(.2)
        raise RuntimeError("Receiver startup timeout")

    running = []
    agent_roots = []
    agent_labels = {}
    try:
        host = receiver()
        running.append(host)
        duplicate = launch(bundle / "lmu-engineer.exe", "--data", root / "receiver", "--http-port", args.http_port,
                           "--ingest-port", args.ingest_port, "--no-browser", "--no-tray")
        assert duplicate.wait(timeout=5) == 0
        for number in range(args.agents):
            data = request("/api/pair", {"endpoint": f"{args.endpoint_host}:{args.ingest_port}", "entry_id": f"Release car {number}",
                                         "label": 'Synthetic, "driver" 0' if number == 0 else f"Synthetic driver {number}"})
            destination = root / f"driver-{number}"
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                assert set(archive.namelist()) == {"lmu-agent.exe", "START-HERE.txt"}
                archive.extractall(destination)
            exe = destination / "lmu-agent.exe"
            raw = exe.read_bytes()
            assert raw[-8:] == b"LMUENG01"
            size = struct.unpack("<I", raw[-12:-8])[0]
            config = json.loads(raw[-12-size:-12])
            assert config["entry_id"] == f"Release car {number}"
            assert config["endpoint"].endswith(str(args.ingest_port))
            agent_labels[config['entry_id']] = config['label']
            assert not (destination / "agent.json").exists()
            agent_roots.append(Path(os.environ["LOCALAPPDATA"]) / "LMUEngineerAgent" / config["source_id"])
            running.append(launch(exe, "--no-tray", "--demo", "--seconds", args.seconds, "--speed", args.speed))
        latencies = []
        deadline = time.monotonic() + args.seconds/args.speed + 60
        next_progress=time.monotonic()
        while any(p.poll() is None for p in running[1:]) and time.monotonic() < deadline:
            before = time.monotonic()
            latest=state()
            latencies.append(time.monotonic() - before)
            if time.monotonic()>=next_progress:
                progress={"at":datetime.datetime.now(datetime.timezone.utc).isoformat(),"cars":len(latest["entries"]),"batches":latest["live_batches"],"game_s":[e["last_sample"]["game_s"] for e in latest["entries"]],"errors":latest["errors"],"state_latency_s":latencies[-1]}
                with (root/"progress.jsonl").open("a",encoding="utf-8") as out:out.write(json.dumps(progress)+"\n")
                next_progress=time.monotonic()+30
            time.sleep(.25 if args.seconds<1000 else 1)
        assert all(p.poll() == 0 for p in running[1:]), "Collector did not drain and exit"
        board = state()
        assert len(board["entries"]) == args.agents
        assert all(e["last_sample"]["game_s"]>=args.seconds-2 for e in board["entries"])
        assert all(len(e["laps"]) >= 3 for e in board["entries"])
        assert all(e["strategy"]["clean_laps"] >= 3 for e in board["entries"])
        assert all(not list(p.joinpath("spool").glob("*.batch")) for p in agent_roots)
        for entry in board["entries"]:
            csv = request("/api/export/" + urllib.parse.quote(entry["entry_id"], safe="")).decode()
            assert csv.startswith("captured_ms,game_s,") and len(csv.splitlines()) > 100
            reader = csv_module.DictReader(io.StringIO(csv))
            assert len(reader.fieldnames) == 118 and len(set(reader.fieldnames)) == 118
            first = next(reader)
            assert None not in first and all(value is not None for value in first.values())
            assert first['entry_id'] == entry['entry_id'] and first['source_id'] == entry['source_id']
            assert first['driver'] == agent_labels[entry['entry_id']], 'CSV text did not round-trip'
            assert first['track'] == 'Monza (SYNTHETIC)' and first['vehicle'] == 'Synthetic GT3'
            assert first['authoritative'] == 'true' and first['valid_lap'] == 'true'
            for corner in ['fl', 'fr', 'rl', 'rr']:
                assert float(first[f'{corner}_pressure_kpa']) == 160
                assert float(first[f'{corner}_temperature_left_c']) == 91
                assert float(first[f'{corner}_temperature_center_c']) == 88
                assert float(first[f'{corner}_temperature_right_c']) == 85
                assert float(first[f'{corner}_brake_c']) >= 400
        first_entry=board["entries"][0]
        recorded_lap=json.loads(request("/api/lap/"+first_entry["laps"][0]["id"]))
        entry_path="/api/laps/"+urllib.parse.quote(first_entry["entry_id"],safe="")
        before_refs=len(json.loads(request(entry_path)))
        imported=json.loads(request("/api/import",recorded_lap))
        assert imported["reference_only"]
        assert len(json.loads(request(entry_path)))==before_refs+1
        unchanged=next(e for e in state()["entries"] if e["entry_id"]==first_entry["entry_id"])
        assert len(unchanged["laps"])==len(first_entry["laps"]),"Imported reference altered live lap evidence"
        native = None
        if args.native_recording:
            info = json.loads(request("/api/native/inspect", {"path": args.native_recording}))
            native = {"channels": len(info["channels"]), "complete_laps": len(info["laps"]), "units": info["unit_hints"]}
        db = sqlite3.connect(root / "receiver" / "race.sqlite")
        before_count = db.execute("select count(*) from batches").fetchone()[0]
        db.close()
        host.terminate()
        host.wait(timeout=10)
        restored = receiver()
        running.append(restored)
        board2 = state()
        assert len(board2["entries"]) == args.agents
        assert all(e["last_received_ms"] == 0 for e in board2["entries"])
        assert sorted(len(e["laps"]) for e in board["entries"]) == sorted(len(e["laps"]) for e in board2["entries"])
        db = sqlite3.connect(root / "receiver" / "race.sqlite")
        assert db.execute("select count(*) from batches").fetchone()[0] == before_count
        db.close()
        report = {"bundle": str(bundle), "batches": before_count, "entries": len(board["entries"]),
                  "simulated_duration_s":args.seconds,"acceleration":args.speed,
                  "collector_endpoint_host":args.endpoint_host,
                  "laps": [len(e["laps"]) for e in board["entries"]], "receiver_errors": board["errors"],
                  "max_state_latency_s": max(latencies), "native": native,
                  "embedded_exe": True, "drained_and_exited": True, "restart_restored_stale_history": True}
        (root / "result.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(json.dumps(report, indent=2))
        assert not board["errors"], "Receiver reported errors; inspect the saved report"
    finally:
        for process in running:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


if __name__ == "__main__":
    main()
