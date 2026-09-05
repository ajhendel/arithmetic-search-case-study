# Matched global-route control follow-up

Selection is fixed from pre-layout results: candidate, no-rule sibling, fastest control-front point, and the nearest control-front points below/above candidate area, per recipe. Duplicate modules are removed. This is a targeted subset, not the full routed control frontier.

| recipe | candidate ps / um2 | fastest selected control ps / um2 | selected controls dominating candidate |
|---|---|---|---:|
| default | 10300 / 9124 | 7810 / 10356 | 0 |
| classic | 7680 / 14418 | 6750 / 15104 | 0 |

Timing uses estimated global-route parasitics, with the same placement flow and loads. Reproduce: `python3 scripts/place_evo608_controls.py` after `python3 scripts/score_evo608_controls.py`.
