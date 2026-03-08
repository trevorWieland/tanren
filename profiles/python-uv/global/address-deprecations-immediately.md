# Address Deprecations Immediately

Deprecation warnings must be addressed immediately. Never defer to future work.

```python
# ✓ Good: Fix deprecation immediately
# Warning: datetime.utcnow() is deprecated since Python 3.12, will be removed in 3.14
# Fix immediately: use datetime.now(timezone.utc)
from datetime import datetime, timezone

current_time = datetime.now(timezone.utc)  # Fixed

# ✗ Bad: Ignore deprecation
import warnings
warnings.filterwarnings("ignore", category=DeprecationWarning)

from datetime import datetime
current_time = datetime.utcnow()  # Will break in Python 3.14
```

**Deprecation handling:**
- Run linters/CI with deprecation warnings as errors
- Fix deprecation warnings immediately when detected
- Replace deprecated APIs with recommended alternatives
- Update dependencies when they deprecate features you use

**Exceptions (extremely rare):**
Defer deprecation fixes only when **both** are true:
1. Blocked on external library release that hasn't shipped the fix yet
2. Fix is documented and tracked with explicit timeline
3. Temporary suppression is localized with clear comment and tracker link

**Never defer for:**
- Internal deprecations in your own code
- Available fixes in current dependencies
- "Will do later" without blocker justification

**CI configuration:**
- Treat deprecation warnings as errors in CI
- Block PRs that introduce new deprecation warnings
- Track external library deprecations in issue tracker

**Why:** Prevents technical debt from accumulating and ensures smooth upgrades when deprecations are removed (no last-minute scrambles when libraries drop deprecated features).
