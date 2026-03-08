# Adapter Interface Protocol

Never access infrastructure adapters directly. Always access storage, models, and external services through **Protocol Interfaces** defined in the Core Domain packages.

```python
# ✓ Good: Access through protocol interface
from core.adapters.vector import VectorStoreProtocol

async def search_context(query: str, vector_store: VectorStoreProtocol):
    """Search vector context via protocol - implementation agnostic."""
    return await vector_store.search(query)

# ✗ Bad: Direct access to implementation
import chromadb

async def search_context(query: str):
    """Search vector context - hardcoded to Chroma."""
    client = chromadb.Client()
    collection = client.get_collection("context")
    return collection.query(query)
```

**Pattern Structure:**
1.  **Define Protocol**: `core.ports.VectorStoreProtocol` (The contract)
2.  **Implement Adapter**: `infrastructure.adapters.chroma.ChromaVectorStore` (The specific tech)
3.  **Inject Dependency**: Pass the adapter where the protocol is expected

**Common Protocols:**
- `VectorStoreProtocol` - Vector storage and retrieval
- `ModelClientProtocol` - LLM model integration
- `StorageProtocol` - Metadata and artifact storage

**Access pattern:**
1.  Define protocol interface in the Core Domain (Ports)
2.  Provide implementation in an Infrastructure package (Adapters)
3.  Inject protocol dependency at runtime
4.  Use only protocol methods - never concrete class

**Why:** Enables swapping implementations (Chroma → pgvector, OpenAI → local models) without changing business logic, makes testing easier with mock protocols, and keeps the core domain clean of infrastructure concerns (Hexagonal/Clean Architecture).
