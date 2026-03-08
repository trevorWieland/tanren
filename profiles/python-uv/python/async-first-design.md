# Async-First Design

Design all APIs and I/O around `async`/`await` and modern structured concurrency.

```python
# ✓ Good: Async-first API
from pydantic import BaseModel

class TranslationRequest(BaseModel):
    scenes: list[str]

async def translate_scenes(request: TranslationRequest) -> list[str]:
    """Translate scenes in parallel using structured concurrency."""
    tasks = [translate_scene(scene) for scene in request.scenes]
    return await asyncio.gather(*tasks)

async def translate_scene(scene: str) -> str:
    """Translate single scene - async for LLM network IO."""
    ...

# ✗ Bad: Blocking I/O
def translate_scenes(request: TranslationRequest) -> list[str]:
    """Translate scenes sequentially - blocks on network IO."""
    results = []
    for scene in request.scenes:
        results.append(translate_scene_sync(scene))  # Blocks
    return results
```

**Why async-first matters:**
- **Parallel execution**: Many phases run agents in parallel (same agent on many scenes at once)
- **Network IO efficiency**: Vast compute requirement is network IO to LLMs; async handles this efficiently
- **Scalability**: Avoid blocking on slow external services
- **Resource efficiency**: Single thread can handle many concurrent network calls

**Async requirements:**
- All I/O operations (LLM calls, storage, vector store, file IO) use `async`/`await`
- Design APIs to be callable from async contexts
- Use modern structured concurrency (`asyncio.gather`, `asyncio.TaskGroup`, etc.)
- Avoid blocking operations in async paths

**Exceptions (entry points only):**
- Script entry points (`main`, CLI commands) may use synchronous wrappers
- Must bridge to async code immediately (e.g., `asyncio.run()`)
- Never block inside async functions

**Why async-first:** Enables parallel execution of agents/scenes, handles network IO to LLMs efficiently, and scales without blocking on slow external services.

**Call-chain tracing for sync boundaries:**

Don't just check the immediate async function body — trace through every sync callee to its leaf I/O. Wrap at the highest sync boundary.

```python
# ✗ Bad: Only checked run_doctor(), missed sync callee
async def run_doctor(config_path: Path) -> Report:
    result = check_config_valid(config_path)  # Looks innocent
    # But check_config_valid() calls open(), toml.load() internally

# ✓ Good: Traced the chain, wrapped at boundary
async def run_doctor(config_path: Path) -> Report:
    result = await asyncio.to_thread(check_config_valid, config_path)
```

This applies to **all filesystem calls**, including cheap ones:
- `Path.exists()`, `Path.stat()`, `Path.is_file()` — can block on network mounts
- `Path.mkdir()`, `Path.unlink()`, `Path.write_bytes()`
- `open()`, `toml.load()`, `.read()`, `.write()`

**Audit procedure:**
1. For each `async def`, list every non-awaited function call
2. For each sync callee, trace to leaf — does it touch the filesystem or network?
3. If yes, wrap the call in `asyncio.to_thread()` at the async boundary
4. If the callee is only ever used from async contexts, consider making it async
