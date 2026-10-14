#!/usr/bin/env python3

import os
import pty
import select
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DRIVE = ROOT / ".cursor" / "skills" / "verify-omalogbook" / "drive.sh"


def fail(message):
    if len(message) > 2000:
        message = message[:1000] + "\n...\n" + message[-800:]
    print(message, file=sys.stderr)
    raise SystemExit(1)


def evidence_names():
    names = []
    for entry in Path("/tmp").iterdir():
        if entry.name.startswith("omalogbook-verify-evidence."):
            names.append(entry.name)
    return set(names)


class Drive:
    def __init__(self, args, env):
        self.master, slave = pty.openpty()
        self.proc = subprocess.Popen(
            args,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            close_fds=True,
            env=env,
        )
        os.close(slave)
        self.buf = b""

    def read_for(self, seconds):
        end = time.time() + seconds
        while time.time() < end:
            timeout = min(0.2, max(0.0, end - time.time()))
            ready, _, _ = select.select([self.master], [], [], timeout)
            if not ready:
                continue
            try:
                chunk = os.read(self.master, 4096)
            except OSError:
                return
            if not chunk:
                return
            self.buf += chunk

    def text(self):
        return self.buf.decode("utf-8", "replace")

    def close(self):
        if self.proc.poll() is None:
            try:
                os.kill(self.proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
        try:
            os.close(self.master)
        except OSError:
            pass
        self.kill_sleeper()

    def kill_sleeper(self):
        for line in self.text().splitlines():
            if line.startswith("sleeper:"):
                pid = line.split(":", 1)[1].strip()
                if pid.isdigit():
                    try:
                        os.kill(int(pid), signal.SIGKILL)
                    except ProcessLookupError:
                        pass


def base_env():
    env = os.environ.copy()
    env.pop("BASH_ENV", None)
    return env


def start_pause(env):
    before = evidence_names()
    drive = Drive(
        ["bash", "--noprofile", "--norc", str(DRIVE), "pause"],
        env,
    )
    deadline = time.time() + 10
    while time.time() < deadline and b"run:" not in drive.buf:
        drive.read_for(0.2)
    if b"run:" not in drive.buf:
        text = drive.text()
        drive.close()
        fail(f"pause did not print run:\n{text}")
    new = evidence_names() - before
    evidence = None
    if len(new) == 1:
        evidence = Path("/tmp") / next(iter(new))
    return drive, evidence


def run_path(text):
    for line in text.splitlines():
        if line.startswith("run:"):
            return line.split(":", 1)[1].strip()
    return ""


def evidence_line(text):
    found = ""
    for line in text.splitlines():
        if line.startswith("evidence:"):
            found = line
    return found


def interrupt(drive):
    sleep_pid = None
    try:
        listing = subprocess.check_output(
            ["ps", "--ppid", str(drive.proc.pid), "-o", "pid=,cmd="],
            text=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        listing = ""
    for line in listing.splitlines():
        parts = line.split(None, 1)
        if len(parts) == 2 and Path(parts[1].split()[0]).name == "sleep":
            sleep_pid = int(parts[0])
    try:
        os.kill(drive.proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    if sleep_pid is not None:
        try:
            os.kill(sleep_pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.time() + 15
    while time.time() < deadline and drive.proc.poll() is None:
        drive.read_for(0.2)
    if drive.proc.poll() is None:
        drive.close()
        fail(f"pause did not exit after TERM\n{drive.text()}")
    drive.read_for(0.5)
    return drive.proc.returncode


def case_sweep():
    env = base_env()
    drive, evidence = start_pause(env)
    run = run_path(drive.text())
    if not run.startswith("/"):
        drive.close()
        fail(f"missing run path\n{drive.text()}")
    pad = "A" * 100000
    victims = []
    try:
        for _ in range(8):
            proc = subprocess.Popen(
                ["sleep", "60"],
                cwd="/var/tmp",
                env={**env, "XDG_RUNTIME_DIR": run, "PAD": pad},
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            victims.append(proc)
        time.sleep(0.3)
        interrupt(drive)
        alive = []
        for proc in victims:
            if proc.poll() is None:
                alive.append(proc.pid)
        if alive:
            fail(f"sweep left runtime-dir processes alive: {alive}")
    finally:
        for proc in victims:
            if proc.poll() is None:
                proc.kill()
        drive.close()
        if evidence is not None:
            subprocess.call(["rm", "-rf", str(evidence)])


def case_bash_env():
    env = base_env()
    cap = Path(tempfile.mkdtemp(prefix="omalogbook-bash-env.", dir="/var/tmp"))
    try:
        count = cap / "count"
        hostile = cap / "hostile.sh"
        hostile.write_text(f"printf 'x\\n' >> {count}\nreadonly BASH_ENV\n")
        run_env = env.copy()
        run_env["BASH_ENV"] = str(hostile)
        drive = Drive(
            ["bash", "--noprofile", "--norc", str(DRIVE), "nosuch"],
            run_env,
        )
        deadline = time.time() + 3
        while time.time() < deadline and drive.proc.poll() is None:
            drive.read_for(0.2)
        if drive.proc.poll() is None:
            text = drive.text()
            drive.close()
            lines = count.read_text() if count.exists() else ""
            fail(f"readonly BASH_ENV did not exit within 3s\ncount={len(lines)}\n{text}")
        drive.read_for(0.3)
        text = drive.text()
        code = drive.proc.returncode
        drive.close()
        lines = count.read_text().splitlines() if count.exists() else []
        if code != 1:
            fail(f"readonly BASH_ENV exit {code}, want 1\n{text}")
        if len(lines) != 1:
            fail(f"readonly BASH_ENV sourced {len(lines)} times\n{text}")
        if "unknown feature" in text:
            fail(f"readonly BASH_ENV kept running\n{text}")

        count.write_text("")
        benign = cap / "benign.sh"
        benign.write_text(f"printf 'x\\n' >> {count}\n")
        run_env["BASH_ENV"] = str(benign)
        drive = Drive(
            ["bash", "--noprofile", "--norc", str(DRIVE), "nosuch"],
            run_env,
        )
        deadline = time.time() + 3
        while time.time() < deadline and drive.proc.poll() is None:
            drive.read_for(0.2)
        if drive.proc.poll() is None:
            text = drive.text()
            drive.close()
            fail(f"BASH_ENV re-exec did not finish\n{text}")
        drive.read_for(0.3)
        text = drive.text()
        code = drive.proc.returncode
        drive.close()
        lines = count.read_text().splitlines() if count.exists() else []
        if code != 2 or "unknown feature" not in text or len(lines) != 1:
            fail(
                f"BASH_ENV re-exec exit {code} sources {len(lines)}\n{text}"
            )
    finally:
        subprocess.call(["rm", "-rf", str(cap)])


def assert_diff_has_marker(root, marker):
    found = list(Path(root).rglob("outside-diff.txt")) if Path(root).is_dir() else []
    if not found:
        return False
    for path in found:
        if marker in path.read_text(errors="replace"):
            return True
    return False


def case_outside():
    env = base_env()
    drive, evidence = start_pause(env)
    run = run_path(drive.text())
    marker = tempfile.mktemp(prefix="omalogbook-outside-write.", dir="/tmp")
    try:
        Path(marker).write_text("planted\n")
        code = interrupt(drive)
        text = drive.text()
        if "wrote outside" not in text:
            fail(f"outside write was not reported\nexit={code}\n{text}")
        if code == 0:
            fail(f"outside write exited 0\n{text}")
        line = evidence_line(text)
        if not line.startswith("evidence: /"):
            fail(f"outside write evidence line {line!r}\n{text}")
        evdir = line.split(" ", 1)[1].strip()
        if not assert_diff_has_marker(evdir, marker):
            fail(f"evidence {evdir} has no outside-diff.txt for {marker}\n{text}")
        if Path(run).exists():
            fail(f"run dir {run} remained after the diff was copied\n{text}")
    finally:
        Path(marker).unlink(missing_ok=True)
        drive.close()
        if evidence is not None:
            subprocess.call(["rm", "-rf", str(evidence)])
        if run:
            subprocess.call(["rm", "-rf", run])


def case_outside_unreadable():
    env = base_env()
    drive, evidence = start_pause(env)
    run = run_path(drive.text())
    marker = tempfile.mktemp(prefix="omalogbook-outside-write.", dir="/tmp")
    try:
        if evidence is None:
            drive.close()
            fail("could not see the evidence directory")
        os.chmod(evidence, 0o555)
        Path(marker).write_text("planted\n")
        code = interrupt(drive)
        text = drive.text()
        if "wrote outside" not in text:
            fail(f"unreadable evidence did not report the write\n{text}")
        if evidence_line(text) != "evidence: none":
            fail(f"unreadable evidence line {evidence_line(text)!r}\n{text}")
        if code == 0:
            fail("unreadable evidence exited 0")
        diff = Path(run) / "outside-diff.txt"
        if not diff.is_file() or marker not in diff.read_text(errors="replace"):
            fail(f"run dir did not keep outside-diff.txt\n{text}")
    finally:
        if evidence is not None:
            try:
                os.chmod(evidence, 0o755)
            except OSError:
                pass
            subprocess.call(["rm", "-rf", str(evidence)])
        Path(marker).unlink(missing_ok=True)
        drive.close()
        if run:
            subprocess.call(["rm", "-rf", run])


def case_unreadable_walk():
    env = base_env()
    blocked = Path(tempfile.mkdtemp(prefix="omalogbook-unreadable.", dir="/tmp"))
    secret = blocked / "secret"
    secret.mkdir()
    os.chmod(secret, 0)
    drive = None
    evidence = None
    run = ""
    try:
        drive, evidence = start_pause(env)
        run = run_path(drive.text())
        code = interrupt(drive)
        text = drive.text()
        if not run.startswith("/"):
            fail(f"unreadable directory aborted pause before run:\n{text}")
        if code != 143:
            fail(f"unreadable walk exit {code}, want 143\n{text}")
    finally:
        try:
            os.chmod(secret, 0o755)
        except OSError:
            pass
        subprocess.call(["rm", "-rf", str(blocked)])
        if drive is not None:
            drive.close()
        if evidence is not None:
            subprocess.call(["rm", "-rf", str(evidence)])
        if run:
            subprocess.call(["rm", "-rf", run])


def case_index():
    env = base_env()
    home = Path(tempfile.mkdtemp(prefix="idxhome.", dir="/tmp"))
    repo = home / "proj"
    script_dir = repo / ".cursor" / "skills" / "verify-omalogbook"
    script_dir.mkdir(parents=True)
    script = script_dir / "drive.sh"
    script.write_bytes(DRIVE.read_bytes())
    script.chmod(0o755)
    commit = [
        "git",
        "-C",
        str(repo),
        "-c",
        "user.email=drive@example.com",
        "-c",
        "user.name=drive",
    ]
    subprocess.check_call(["git", "-C", str(repo), "init", "-q"])
    subprocess.check_call([*commit, "add", "-A"])
    subprocess.check_call([*commit, "commit", "-q", "-m", "init"])
    env["HOME"] = str(home)
    before = evidence_names()
    drive = Drive(["bash", "--noprofile", "--norc", str(script), "pause"], env)
    evidence = None
    run = ""
    try:
        deadline = time.time() + 10
        while time.time() < deadline and b"run:" not in drive.buf:
            drive.read_for(0.2)
        if b"run:" not in drive.buf:
            fail(f"copied pause did not print run:\n{drive.text()}")
        new = evidence_names() - before
        if len(new) == 1:
            evidence = Path("/tmp") / next(iter(new))
            os.chmod(evidence, 0o555)
        time.sleep(1.3)
        run = run_path(drive.text())
        interrupt(drive)
        text = drive.text()
        diff = Path(run) / "outside-diff.txt" if run else Path("/no/such")
        body = diff.read_text(errors="replace") if diff.is_file() else ""
        if ".git/index" in body:
            fail(f"git status index mtime counted as an outside write\n{body}")
        if "wrote outside" in text and not body:
            fail(f"wrote outside and the diff was deleted\n{text}")
    finally:
        if evidence is not None:
            try:
                os.chmod(evidence, 0o755)
            except OSError:
                pass
            subprocess.call(["rm", "-rf", str(evidence)])
        drive.close()
        subprocess.call(["rm", "-rf", str(home)])
        if run:
            subprocess.call(["rm", "-rf", run])


CASES = {
    "sweep": case_sweep,
    "bash-env": case_bash_env,
    "outside": case_outside,
    "outside-unreadable": case_outside_unreadable,
    "index": case_index,
    "unreadable-walk": case_unreadable_walk,
}


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in CASES:
        fail("usage: drive_guards.py " + "|".join(CASES))
    if not DRIVE.is_file():
        fail(f"missing {DRIVE}")
    CASES[sys.argv[1]]()


if __name__ == "__main__":
    main()
