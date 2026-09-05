#!/usr/bin/env python3
"""Plot the measured default-flow frontier before/after the bounded comparator."""
import os
os.environ['OMP_NUM_THREADS'] = '1'
os.environ['OPENBLAS_NUM_THREADS'] = '1'
import csv
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

root = Path('results/discovery/mac_ufomac_challenge_20260905')
rows = list(csv.DictReader((root / 'default_combined_frontier.tsv').open(), delimiter='\t'))
candidate = next(r for r in rows if r['class'] == 'candidate')
winner = next(r for r in rows if r['module'] == 'mul24_mac_ufomac_brentkung_mux_default')
fig, axes = plt.subplots(1, 2, figsize=(10, 4.5), layout='constrained')
for ax in axes:
    for cls, color, marker, label in [
        ('prior', '#777777', 'o', 'Earlier baselines'),
        ('ufomac_bounded', '#2878b5', 's', 'Bounded UFO-MAC-derived controls')]:
        group = [r for r in rows if (r['class'] == cls if cls != 'prior'
                                    else r['class'] not in ('candidate', 'ufomac_bounded'))]
        ax.scatter([float(r['area_um2']) for r in group],
                   [float(r['worst_arrival_ps']) / 1000 for r in group],
                   c=color, marker=marker, s=24, alpha=.75, label=label)
    ax.scatter(float(candidate['area_um2']), float(candidate['worst_arrival_ps'])/1000,
               c='#c43b39', marker='X', s=100, label='evo608 (now dominated)', zorder=4)
    ax.scatter(float(winner['area_um2']), float(winner['worst_arrival_ps'])/1000,
               c='#00856a', marker='*', s=160, label='New frontier point', zorder=5)
    ax.set(xlabel='Cell area (µm²)', ylabel='Global-route delay estimate (ns)')
    ax.grid(alpha=.2)
axes[0].set_title('Combined comparison: 51 designs')
axes[1].set(title='Detail near evo608', xlim=(19550, 20500), ylim=(13.3, 22.5))
axes[1].annotate('evo608: 15.25 ns / 19,884 µm²', (19884, 15.25),
                 xytext=(19990, 16.3), fontsize=8,
                 arrowprops={'arrowstyle': '-', 'color': '#c43b39'})
axes[1].annotate('New: 14.01 ns / 19,843 µm²', (19843, 14.01),
                 xytext=(19600, 13.5), fontsize=8)
handles, labels = axes[0].get_legend_handles_labels()
fig.legend(handles, labels, loc='outside lower center', ncols=2, fontsize=8)
fig.suptitle('Default SKY130 flow, utilization 45% — lower and left is better')
for suffix in ('svg', 'png'):
    fig.savefig(root/f'frontier.{suffix}', dpi=160)
print('Wrote frontier.svg and frontier.png from the audited result table')
