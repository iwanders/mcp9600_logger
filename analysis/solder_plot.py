#!/usr/bin/env python3

import matplotlib.pyplot as plt
import numpy as np


def load_temps(fname):
    entries = []
    with open(fname) as f:
        for k in f.readlines():
            if "," in k:
                stamp, temp = [float(a.strip()) for a in k.strip().split(",")]
                entries.append((stamp, temp))
    return entries

def t(a):
    m, s = (float(x.strip()) for x in a.split(":"))
    return m * 60.0 + s

temp_level = [
    (t("0:0"), True),
    (t("1:12"), False),
    (t("2:13"), True),
    (t("3:33"), False),
]
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

    d = load_temps("log.txt")
    d = np.array(d)
    plt.plot(d[:,0], d[:, 1])
    plt.xlabel("time (s)")
    plt.ylabel("temp (C)")
    #plt.show()

    ts = np.array(add_temps(temp_level))
    tshift = 19 * 1000
    plt.plot(ts[:,0] * 1000.0 + tshift, ts[:, 1] * 200, "r")


    ds = np.array(solder_reflow_ts391AX50)
    plt.plot(ds[:,0] * 1000.0 + tshift + 30e3, ds[:, 1] , "g")
    plt.show()
