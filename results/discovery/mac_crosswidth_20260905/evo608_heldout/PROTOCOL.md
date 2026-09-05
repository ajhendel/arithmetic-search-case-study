# Frozen held-out-width comparison

Recorded before generation and physical evaluation: 2026-09-05T07:24:46.340217+00:00.
This is a local prospective protocol, not an externally registered preregistration.

Candidate: evo608 from seed 1835098964; genome SHA256 `de7e3943afd7c0d37a93de6baec583d6c8214195e6a035451b388b84f5d21bc4`.
No mutation, rule editing, or selection using the new results is allowed in this
comparison. Widths: 12 and 24. Recipes: default and classic, flattened mapping.
Use the same SKY130 HD typical Liberty and STA loads as the 8/16-bit challenge.

At each width generate the candidate, exact no-rule sibling, and all 30 textbook
trunks with its frozen root/fusion plan. Verify each candidate and sibling on
200,000 deterministic samples plus fixed carry/round/saturation boundary cases;
controls receive 1,000 samples plus the same fixed cases. Use the u128 output
path so every bit of the W=24 bundle is checked. Formal correctness is not
implied by these samples.

Report all 128 mapped forms. Primary endpoints: candidate delay/area change
relative to its sibling for every width/recipe and count of matched controls
that dominate it. A consistent rule improvement requires lower delay with no
area increase in every declared comparison. Mixed outcomes are retained as
mixed, not relabelled success. No claim of a global physical winner follows
from this same-root-plan control matrix. Further routing is a separate step.

Historical W=8/16 values were inspected during selection and are not held out.
