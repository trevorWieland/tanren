# Three-Tier BDD Structure

All executable tests are scenario-driven and organized by runtime tier. Each tier still uses `.feature` files and BDD step bindings.

```
tests/
├── unit/
│   ├── features/
│   │   └── processor.feature
│   └── steps/
│       └── test_processor_steps.py
├── integration/
│   ├── features/
│   │   └── pipeline.feature
│   └── steps/
│       └── test_pipeline_steps.py
└── quality/
    ├── features/
    │   └── model_quality.feature
    └── steps/
        └── test_model_quality_steps.py
```

**Tier definitions:**
- `unit`: <250ms per scenario, isolated logic, no external services
- `integration`: <5s per scenario, real services, mocked model adapters
- `quality`: <30s per scenario, real services, real model adapters

**Rules:**
- All scenarios live in `features/*.feature`
- All step bindings live in `steps/test_*_steps.py`
- No free-form executable tests outside scenario bindings
- Every scenario has a behavior tag and a tier tag

**Why:** Runtime tiers remain useful for speed and environment control while preserving a single behavior-first testing model.
