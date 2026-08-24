#!/usr/bin/env python3
"""Run the probe inside a real VTE terminal, headlessly.

VTE is the engine behind GNOME Terminal, Tilix, Terminator, xfce4-terminal and
Guake, so whatever it answers is what a very large share of Linux users see.

    xvfb-run -a python3 probe/drivers/vte_driver.py <probe-binary> <out.tsv>
"""
import os
import sys

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Vte", "2.91")
from gi.repository import Gtk, Vte, GLib  # noqa: E402

probe_bin, out_path = sys.argv[1], sys.argv[2]
status = {"code": None}


def on_exit(_term, code):
    status["code"] = code
    app.quit()


def activate(application):
    win = Gtk.ApplicationWindow(application=application)
    term = Vte.Terminal()
    # A wide grid, so nothing under test ever wraps and skews the column report.
    term.set_size(200, 24)
    win.add(term)
    env = [f"{k}={v}" for k, v in os.environ.items() if k != "TERM"]
    env.append("TERM=xterm-256color")
    term.spawn_async(
        Vte.PtyFlags.DEFAULT, os.getcwd(),
        [probe_bin, out_path], env,
        GLib.SpawnFlags.DEFAULT, None, None, -1, None, None, None,
    )
    term.connect("child-exited", on_exit)
    win.show_all()
    # Never hang a CI job on a terminal that stops answering.
    GLib.timeout_add_seconds(120, lambda: (app.quit(), False)[1])


app = Gtk.Application(application_id="dev.cellwidth.probe")
app.connect("activate", activate)
app.run([])
sys.exit(0 if status["code"] == 0 else 1)
