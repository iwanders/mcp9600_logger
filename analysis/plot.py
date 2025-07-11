#!/usr/bin/env python3

import matplotlib.pyplot as plt
import numpy as np
import argparse


def load_log(fname):
    entries = []
    with open(fname) as f:
        for k in f.readlines():
            if "," in k:
                stamp, temp = [float(a.strip()) for a in k.strip().split(",")]
                entries.append((stamp / 1000.0, temp))
    return entries

def t(a):
    if ":" in a:
        m, s = (float(x.strip()) for x in a.split(":"))
    else:
        return float(a.split())
    return m * 60.0 + s

def load_heat(fname, temps):
    time_entries = []
    interval_entries = []
    with open(fname) as f:
        for k in f.readlines():
            if "," in k:
                stamp, value = [a.strip() for a in k.strip().split(",")]
                time_entries.append((t(stamp), float(value)))
            if ":" in k:
                interval, value = [a.strip() for a in k.strip().split(":")]
                start,end = [float(a.strip()) for a in interval.split("-")]
                interval_entries.append((start, end, float(value)))
    
    if time_entries:
        return add_temps(time_entries)

    if interval_entries:
        temp_entries = []
        interval_index = 0
        temp_entries.append((temps[0][0], 0))
        for ts, temp in temps:
            if interval_index >= len(interval_entries):
                break;
            lower, upper, value = interval_entries[interval_index]
            if lower < upper:
                # rising
                if lower < temp:
                    # add previous point.
                    temp_entries.append((ts, temp_entries[-1][1]))
                    # add new point.
                    temp_entries.append((ts, value))
                    interval_index += 1
            else:
                # lowering
                if temp < upper:
                    # add previous point.
                    temp_entries.append((ts, temp_entries[-1][1]))
                    # add new point.
                    temp_entries.append((ts, value))
                    interval_index += 1
                    
                    
        return np.array(temp_entries)



def add_temps(temps):
    new_entries = [(0, False)]
    for k in temps:
        if len(new_entries) == 0:
            new_entries.append(k)
            continue
        t, v = k
        new_entries.append((t, new_entries[-1][1]))
        new_entries.append(k)
    return new_entries

solder_reflow_ts391AX50 = [
    (0, 25),
    (30, 100),
    (120, 150),
    (150, 183),
    (210, 235),
    (240, 183),
]

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("log", help="Temperature log from the logger, from cat /dev/ttyACM* >> log.txt ")

    parser.add_argument("--heat", default=None, help="Heating log, 'm:s, 0..1\n...' OR '0-60: 1\n60-100:0\n..\n240-0:0' ")
    parser.add_argument("--heat-shift", default=0.0, type=float, help="Shift heating by this time.")

    parser.add_argument("--reflow-profile", default=False, action="store_true", help="Plot TS391AX50 reflow profile")

    parser.add_argument("--no-show", default=True, action="store_false", dest="show", help="Prevent showing the figure")
    parser.add_argument("--save", default=None, help="Save the figure to this file.")
    
    args = parser.parse_args()

    d = load_log(args.log)
    d = np.array(d)
    plt.plot(d[:,0], d[:, 1])
    plt.xlabel("time (s)")
    plt.ylabel("temp (C)")
    #plt.show()

    tmax = np.max(d[:, 1])

    if args.heat:
        ts = np.array(load_heat(args.heat, d))
        plt.plot(ts[:,0] + args.heat_shift, ts[:, 1] * tmax, "r")

    if args.reflow_profile:
        # Find peak.
        highest_index = np.argmax(d[:, 1])
        time_at_index = d[:, 0][np.argmax(d[:, 1])]
        s = np.array(solder_reflow_ts391AX50)
        profile_highest = np.argmax(s[:, 1])
        profile_time = s[:,0][np.argmax(s[:, 1])]
        # then shift it;
        reflow_shift = time_at_index - profile_time
        # And plot it.
        plt.plot(s[:,0] + reflow_shift, s[:, 1] , "g")

    if args.show:
        plt.show()

    if args.save:
        plt.savefig(args.save)
