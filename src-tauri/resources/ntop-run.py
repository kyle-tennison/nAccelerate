#!/usr/bin/env python3
"""Run and save the .ntop notebook in a directory with ntopcl inside the nTop Docker image.

nAccelerate installs this on the server and runs it for every job. It reads the login from
NTOP_EMAIL / NTOP_PASSWORD and the requested resources from NACCEL_* variables, and prints
"Saved: <path>" when the notebook was saved.
"""
import argparse
import grp
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path


def fail(message):
    print(f"nAccelerate: {message}", flush=True)
    sys.exit(1)


def in_docker_group():
    try:
        return grp.getgrnam("docker").gr_gid in os.getgroups()
    except KeyError:
        return False


def docker(args):
    """Run a docker command, through `sg docker` when this shell is not in the docker group yet."""
    if in_docker_group():
        return ["docker", *args]
    return ["sg", "docker", "-c", shlex.join(["docker", *args])]


def find_image():
    if os.environ.get("NTOP_IMAGE"):
        return os.environ["NTOP_IMAGE"]
    out = subprocess.run(docker(["images", "--format", "{{.Repository}}:{{.Tag}}"]), capture_output=True, text=True)
    images = [i for i in out.stdout.split() if "ntop" in i.lower() and not i.endswith(":<none>")]
    if not images:
        fail("No nTop Docker image found on this server. Set NTOP_IMAGE to its name.")
    return images[0]


parser = argparse.ArgumentParser(description="Run and save the .ntop notebook in a directory.")
parser.add_argument("--input", "-i", required=True, help="directory containing one .ntop file")
work = Path(parser.parse_args().input).resolve()
notebooks = list(work.glob("*.ntop"))
if len(notebooks) != 1:
    fail(f"Expected exactly one .ntop file in {work}, found {len(notebooks)}")
notebook = notebooks[0]

env = os.environ
email, password = env.get("NTOP_EMAIL", ""), env.get("NTOP_PASSWORD", "")
if not email or not password:
    fail("Add your nTop login in nAccelerate's settings.")

limits = []
if env.get("NACCEL_JOB"):
    limits += ["--name", env["NACCEL_JOB"]]
if env.get("NACCEL_CPUSET"):
    limits += ["--cpuset-cpus", env["NACCEL_CPUSET"]]
if env.get("NACCEL_MEM_GB"):
    limits += ["--memory", f"{env['NACCEL_MEM_GB']}g"]
if int(env.get("NACCEL_GPUS") or 0) > 0:
    limits += ["--gpus", env["NACCEL_GPUS"]]

user = env.get("USER") or env.get("LOGNAME") or "ntop"
log_name = f"ntopcl_{time.strftime('%Y%m%d_%H%M%S')}.log"
print(f"Log: {work / log_name}", flush=True)

result = subprocess.run(
    docker([
        "run", "--rm", *limits,
        "-e", f"USER={user}", "-e", f"LOGNAME={user}",  # ntopcl --save segfaults without these
        "-v", f"{work}:/work", "-w", "/work",
        find_image(),
        "ntopcl", "-u", email, "-w", password, "-v", "2", "-s",
        f"--logfile={log_name}", notebook.name,
    ]),
    stdin=subprocess.DEVNULL,
)

print(f"Saved: {notebook}" if result.returncode == 0 else f"ntopcl failed (exit {result.returncode})", flush=True)
sys.exit(result.returncode)
