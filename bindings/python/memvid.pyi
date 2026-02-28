"""Type stubs for the memvid native Python extension.

memvid provides crash-safe, deterministic, single-file AI memory
backed by a Rust core via PyO3 bindings.
"""

from __future__ import annotations

from typing import Dict, List, Optional, Tuple

def version() -> str:
    """Return the memvid-core version string."""
    ...

class MemvidError(Exception):
    """Raised for all memvid errors.

    Attributes:
        message: Human-readable error description.
        code: Machine-readable error code string (e.g. ``"IO"``, ``"LOCKED"``).
        path: Optional filesystem path associated with the error.
    """

    message: str
    code: str
    path: Optional[str]

# ---------------------------------------------------------------------------
# Core handle
# ---------------------------------------------------------------------------

class Memvid:
    """Handle to a ``.mv2`` memory file.

    Use the static factory methods :meth:`create`, :meth:`open`, or
    :meth:`open_read_only` to obtain a handle.  Supports the context-manager
    protocol (``with Memvid.create(path) as mv: ...``).
    """

    # -- lifecycle (static) -------------------------------------------------

    @staticmethod
    def create(path: str) -> Memvid:
        """Create a new ``.mv2`` file at *path*."""
        ...

    @staticmethod
    def open(path: str) -> Memvid:
        """Open an existing ``.mv2`` file with exclusive read-write access."""
        ...

    @staticmethod
    def open_read_only(path: str) -> Memvid:
        """Open an existing ``.mv2`` file in read-only mode."""
        ...

    def close(self) -> None:
        """Close the handle and release the file lock."""
        ...

    def __enter__(self) -> Memvid: ...
    def __exit__(
        self,
        exc_type: object,
        exc_val: object,
        exc_tb: object,
    ) -> bool: ...

    # -- properties ---------------------------------------------------------

    @property
    def is_read_only(self) -> bool:
        """Whether the handle was opened in read-only mode."""
        ...

    @property
    def path(self) -> str:
        """Filesystem path of the ``.mv2`` file."""
        ...

    @property
    def frame_count(self) -> int:
        """Total number of frames in the file."""
        ...

    @property
    def next_frame_id(self) -> int:
        """The frame ID that will be assigned to the next appended frame."""
        ...

    @property
    def memory_card_count(self) -> int:
        """Total number of memory cards."""
        ...

    # -- write --------------------------------------------------------------

    def put_bytes(self, data: bytes) -> int:
        """Append raw *data* and return the new frame ID."""
        ...

    def put_bytes_with_options(
        self,
        data: bytes,
        *,
        timestamp: Optional[int] = None,
        track: Optional[str] = None,
        kind: Optional[str] = None,
        uri: Optional[str] = None,
        title: Optional[str] = None,
        tags: Optional[List[str]] = None,
        labels: Optional[List[str]] = None,
        search_text: Optional[str] = None,
        enable_embedding: bool = False,
        auto_tag: bool = True,
        dedup: bool = False,
        role: Optional[str] = None,
        source_path: Optional[str] = None,
        no_raw: bool = False,
    ) -> int:
        """Append raw *data* with metadata options and return the frame ID."""
        ...

    def put_with_embedding(self, data: bytes, embedding: List[float]) -> int:
        """Append *data* together with a pre-computed *embedding* vector."""
        ...

    def delete_frame(self, frame_id: int) -> int:
        """Mark *frame_id* as deleted and return the tombstone frame ID."""
        ...

    def begin_batch(
        self,
        *,
        compression_level: int = 3,
        disable_auto_checkpoint: bool = True,
        skip_sync: bool = False,
        wal_pre_size_bytes: int = 0,
    ) -> None:
        """Enter batch-write mode with the given options."""
        ...

    def end_batch(self) -> None:
        """Exit batch-write mode and flush pending writes."""
        ...

    def commit(self) -> None:
        """Flush the WAL and rebuild indexes."""
        ...

    def commit_skip_indexes(self) -> None:
        """Flush the WAL without rebuilding indexes."""
        ...

    def finalize_indexes(self) -> None:
        """Rebuild all indexes (call after bulk ingestion with ``commit_skip_indexes``)."""
        ...

    # -- search -------------------------------------------------------------

    def search(
        self,
        query: str,
        *,
        top_k: int = 10,
        snippet_chars: int = 200,
        uri: Optional[str] = None,
        scope: Optional[str] = None,
        cursor: Optional[str] = None,
        no_sketch: bool = False,
    ) -> SearchResponse:
        """Run a full-text search and return ranked results."""
        ...

    # -- ask / RAG ----------------------------------------------------------

    def ask(
        self,
        question: str,
        *,
        top_k: int = 5,
        snippet_chars: int = 200,
        uri: Optional[str] = None,
        scope: Optional[str] = None,
        context_only: bool = False,
        mode: str = "hybrid",
    ) -> AskResponse:
        """Retrieval-augmented Q&A powered by the memvid file."""
        ...

    # -- timeline -----------------------------------------------------------

    def timeline(
        self,
        *,
        limit: Optional[int] = None,
        since: Optional[int] = None,
        until: Optional[int] = None,
        reverse: bool = False,
    ) -> List[TimelineEntry]:
        """Scan frames chronologically."""
        ...

    # -- frame access -------------------------------------------------------

    def frame_by_id(self, frame_id: int) -> Frame:
        """Look up a frame by its numeric ID."""
        ...

    def frame_by_uri(self, uri: str) -> Frame:
        """Look up a frame by its URI."""
        ...

    def frame_canonical_payload(self, frame_id: int) -> bytes:
        """Return the decompressed payload of *frame_id*."""
        ...

    def frame_text_by_id(self, frame_id: int) -> str:
        """Return the full text content of *frame_id*."""
        ...

    def frame_preview_by_id(self, frame_id: int) -> str:
        """Return a truncated preview of *frame_id*."""
        ...

    def frame_embedding(self, frame_id: int) -> Optional[List[float]]:
        """Return the embedding vector for *frame_id*, or ``None``."""
        ...

    def frame_context(self, frame_id: int, query: str) -> FrameContext:
        """Return context text and match count for *frame_id* given *query*."""
        ...

    # -- blob streaming -----------------------------------------------------

    def blob_reader(self, frame_id: int) -> BlobReader:
        """Open a streaming reader for *frame_id*."""
        ...

    def blob_reader_by_uri(self, uri: str) -> BlobReader:
        """Open a streaming reader for the frame identified by *uri*."""
        ...

    # -- memory cards: write ------------------------------------------------

    def put_memory_card(
        self,
        *,
        kind: str,
        entity: str,
        slot: str,
        value: str,
        polarity: Optional[str] = None,
        event_date: Optional[int] = None,
    ) -> int:
        """Insert a single memory card and return its card ID."""
        ...

    def put_memory_cards(self, cards: List[dict]) -> List[int]:
        """Batch-insert memory cards from a list of dicts."""
        ...

    def clear_memories(self) -> None:
        """Delete all memory cards."""
        ...

    # -- memory cards: query ------------------------------------------------

    def get_current_memory(self, entity: str, slot: str) -> Optional[MemoryCard]:
        """Return the latest card for *entity* / *slot*, or ``None``."""
        ...

    def get_entity_memories(self, entity: str) -> List[MemoryCard]:
        """Return all memory cards for *entity*."""
        ...

    def get_memory_timeline(self, entity: str) -> List[MemoryCard]:
        """Return event-type cards for *entity* in chronological order."""
        ...

    def get_preferences(self, entity: str) -> List[MemoryCard]:
        """Return preference cards for *entity*."""
        ...

    def aggregate_memory_slot(self, entity: str, slot: str) -> List[str]:
        """Return unique values for *entity* / *slot*."""
        ...

    def memories_stats(self) -> MemoriesStats:
        """Return statistics about the memories track."""
        ...

    # -- schema -------------------------------------------------------------

    def register_schema(
        self,
        *,
        name: str,
        description: Optional[str] = None,
        domain: Optional[List[str]] = None,
        range: str = "string",
        range_entity_kind: Optional[str] = None,
        range_enum_values: Optional[List[str]] = None,
        cardinality: str = "single",
        inverse: Optional[str] = None,
        builtin: bool = False,
    ) -> None:
        """Register a predicate schema for memory card validation."""
        ...

    def validate_card(self, card: dict) -> ValidationResult:
        """Validate a memory card dict against registered schemas."""
        ...

    def set_schema_strict(self, strict: bool) -> None:
        """Enable or disable strict schema validation."""
        ...

    def infer_schemas(self) -> List[PredicateSchema]:
        """Auto-detect schemas from existing memory cards."""
        ...

    # -- logic mesh ---------------------------------------------------------

    def add_mesh_node(
        self,
        *,
        canonical_name: str,
        display_name: str,
        kind: str,
        confidence: float = 0.9,
    ) -> None:
        """Add an entity node to the logic mesh."""
        ...

    def add_mesh_nodes(self, nodes: List[dict]) -> None:
        """Batch-add entity nodes from a list of dicts."""
        ...

    def add_mesh_edge(
        self,
        *,
        from_node: int,
        to_node: int,
        link: str,
        confidence: float = 0.9,
        frame_id: Optional[int] = None,
    ) -> None:
        """Add a directed edge between two entity nodes."""
        ...

    def add_mesh_edges(self, edges: List[dict]) -> None:
        """Batch-add edges from a list of dicts."""
        ...

    def follow(self, start: str, link: str, hops: int) -> List[FollowResult]:
        """Traverse the entity graph from *start* along *link* for *hops* hops."""
        ...

    def find_entity(self, name: str) -> Optional[MeshNode]:
        """Find an entity by canonical or display name."""
        ...

    def frame_entities(self, frame_id: int) -> List[MeshNode]:
        """Return entity nodes mentioned in *frame_id*."""
        ...

    def entities_by_kind(self, kind: str) -> List[MeshNode]:
        """Return all entity nodes of the given *kind*."""
        ...

    def logic_mesh_stats(self) -> LogicMeshStats:
        """Return statistics about the logic mesh."""
        ...

    # -- sketches -----------------------------------------------------------

    def build_all_sketches(self, variant: str = "small") -> int:
        """Build sketch indexes and return the number of entries created."""
        ...

    def find_sketch_candidates(self, query: str, top_k: int) -> List[int]:
        """Return frame IDs of approximate matches via sketch search."""
        ...

    def has_sketches(self) -> bool:
        """Return whether sketch indexes have been built."""
        ...

    def sketch_stats(self) -> SketchTrackStats:
        """Return statistics about the sketch track."""
        ...

    # -- enrichment ---------------------------------------------------------

    def start_enrichment_worker(
        self,
        *,
        embedding_batch_size: int = 32,
        checkpoint_interval: int = 100,
        task_delay_ms: int = 50,
        max_task_time_ms: int = 5000,
    ) -> EnrichmentHandle:
        """Start a background enrichment worker."""
        ...

    def get_unenriched_frames(
        self,
        engine_kind: str,
        engine_version: str,
    ) -> List[int]:
        """Return frame IDs that have not been enriched by *engine_kind*/*engine_version*."""
        ...

    def record_enrichment(
        self,
        frame_id: int,
        engine_kind: str,
        engine_version: str,
        card_ids: Optional[List[int]] = None,
    ) -> None:
        """Record that *frame_id* has been enriched."""
        ...

    def is_frame_enriched(
        self,
        frame_id: int,
        engine_kind: str,
        engine_version: str,
    ) -> bool:
        """Check whether *frame_id* has been enriched by the given engine."""
        ...

    # -- document readers ---------------------------------------------------

    def preview_chunks(self, data: bytes) -> Optional[List[str]]:
        """Preview how *data* would be chunked."""
        ...

    # -- maintenance (static) -----------------------------------------------

    @staticmethod
    def verify(path: str, *, deep: bool = False) -> VerificationReport:
        """Verify integrity of the ``.mv2`` file at *path*."""
        ...

    @staticmethod
    def doctor(
        path: str,
        *,
        rebuild_time_index: bool = False,
        rebuild_lex_index: bool = False,
        rebuild_vec_index: bool = False,
        vacuum: bool = False,
        dry_run: bool = False,
        quiet: bool = False,
    ) -> DoctorReport:
        """Run diagnostics and repair on the file at *path*."""
        ...

    @staticmethod
    def doctor_plan(
        path: str,
        *,
        rebuild_time_index: bool = False,
        rebuild_lex_index: bool = False,
        rebuild_vec_index: bool = False,
        vacuum: bool = False,
        dry_run: bool = False,
        quiet: bool = False,
    ) -> DoctorPlan:
        """Generate a repair plan without applying it."""
        ...

    @staticmethod
    def doctor_apply(path: str, plan: DoctorPlan) -> DoctorReport:
        """Apply a previously generated *plan* to the file at *path*."""
        ...

    def vacuum(self) -> None:
        """Compact the file by removing deleted frames."""
        ...

    def stats(self) -> Stats:
        """Return comprehensive file statistics."""
        ...

# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

class Frame:
    """Metadata for a single frame."""

    @property
    def id(self) -> int: ...
    @property
    def timestamp(self) -> int: ...
    @property
    def kind(self) -> Optional[str]: ...
    @property
    def track(self) -> Optional[str]: ...
    @property
    def uri(self) -> Optional[str]: ...
    @property
    def title(self) -> Optional[str]: ...
    @property
    def tags(self) -> List[str]: ...
    @property
    def labels(self) -> List[str]: ...
    @property
    def role(self) -> str: ...
    @property
    def canonical_encoding(self) -> str: ...
    @property
    def search_text(self) -> Optional[str]: ...
    @property
    def extra_metadata(self) -> Dict[str, str]: ...
    def __repr__(self) -> str: ...

class FrameContext:
    """Context text extracted for a query against a specific frame."""

    @property
    def text(self) -> str: ...
    @property
    def match_count(self) -> int: ...
    def __repr__(self) -> str: ...

class BlobReader:
    """Streaming reader for large frame payloads.

    Supports the context-manager protocol and iteration::

        with mv.blob_reader(frame_id) as r:
            for chunk in r:
                process(chunk)
    """

    @property
    def length(self) -> int:
        """Total payload size in bytes."""
        ...

    def read(self, size: int = -1) -> bytes:
        """Read up to *size* bytes (all remaining if *size* < 0)."""
        ...

    def close(self) -> None:
        """Release underlying resources."""
        ...

    def __enter__(self) -> BlobReader: ...
    def __exit__(
        self,
        exc_type: object,
        exc_val: object,
        exc_tb: object,
    ) -> bool: ...
    def __iter__(self) -> BlobReader: ...
    def __next__(self) -> bytes: ...

class TimelineEntry:
    """A single entry from a timeline scan."""

    @property
    def frame_id(self) -> int: ...
    @property
    def timestamp(self) -> int: ...
    @property
    def preview(self) -> str: ...
    @property
    def uri(self) -> Optional[str]: ...
    @property
    def child_frames(self) -> List[int]: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Search
# ---------------------------------------------------------------------------

class SearchHit:
    """A single hit from a search query."""

    @property
    def rank(self) -> int: ...
    @property
    def frame_id(self) -> int: ...
    @property
    def uri(self) -> str: ...
    @property
    def title(self) -> Optional[str]: ...
    @property
    def text(self) -> str: ...
    @property
    def score(self) -> Optional[float]: ...
    @property
    def chunk_text(self) -> Optional[str]: ...
    @property
    def match_count(self) -> int: ...
    def __repr__(self) -> str: ...

class SearchResponse:
    """Result of a :meth:`Memvid.search` call."""

    @property
    def query(self) -> str: ...
    @property
    def elapsed_ms(self) -> int: ...
    @property
    def total_hits(self) -> int: ...
    @property
    def hits(self) -> List[SearchHit]: ...
    @property
    def context(self) -> str: ...
    @property
    def next_cursor(self) -> Optional[str]: ...
    @property
    def engine(self) -> str: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Ask / RAG
# ---------------------------------------------------------------------------

class AskStats:
    """Timing statistics for an ask operation."""

    @property
    def retrieval_ms(self) -> int: ...
    @property
    def synthesis_ms(self) -> int: ...
    @property
    def latency_ms(self) -> int: ...
    def __repr__(self) -> str: ...

class AskCitation:
    """A citation returned by an ask operation."""

    @property
    def index(self) -> int: ...
    @property
    def frame_id(self) -> int: ...
    @property
    def uri(self) -> str: ...
    @property
    def chunk_range_start(self) -> Optional[int]: ...
    @property
    def chunk_range_end(self) -> Optional[int]: ...
    @property
    def score(self) -> Optional[float]: ...
    def __repr__(self) -> str: ...

class AskContextFragment:
    """A context fragment used during RAG synthesis."""

    @property
    def rank(self) -> int: ...
    @property
    def frame_id(self) -> int: ...
    @property
    def uri(self) -> str: ...
    @property
    def title(self) -> Optional[str]: ...
    @property
    def score(self) -> Optional[float]: ...
    @property
    def match_count(self) -> int: ...
    @property
    def text(self) -> str: ...
    @property
    def kind(self) -> Optional[str]: ...
    def __repr__(self) -> str: ...

class AskResponse:
    """Result of a :meth:`Memvid.ask` call."""

    @property
    def question(self) -> str: ...
    @property
    def mode(self) -> str: ...
    @property
    def retriever(self) -> str: ...
    @property
    def context_only(self) -> bool: ...
    @property
    def answer(self) -> Optional[str]: ...
    @property
    def retrieval(self) -> SearchResponse: ...
    @property
    def citations(self) -> List[AskCitation]: ...
    @property
    def context_fragments(self) -> List[AskContextFragment]: ...
    @property
    def stats(self) -> AskStats: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Memory cards
# ---------------------------------------------------------------------------

class MemoryCard:
    """A structured memory card."""

    @property
    def id(self) -> int: ...
    @property
    def kind(self) -> str: ...
    @property
    def entity(self) -> str: ...
    @property
    def slot(self) -> str: ...
    @property
    def value(self) -> str: ...
    @property
    def polarity(self) -> Optional[str]: ...
    @property
    def event_date(self) -> Optional[int]: ...
    @property
    def document_date(self) -> Optional[int]: ...
    @property
    def version_key(self) -> Optional[str]: ...
    @property
    def version_relation(self) -> str: ...
    @property
    def source_frame_id(self) -> int: ...
    @property
    def source_uri(self) -> Optional[str]: ...
    @property
    def engine(self) -> str: ...
    @property
    def engine_version(self) -> str: ...
    @property
    def confidence(self) -> Optional[float]: ...
    @property
    def created_at(self) -> int: ...
    def __repr__(self) -> str: ...

class MemoriesStats:
    """Statistics about the memories track."""

    @property
    def card_count(self) -> int: ...
    @property
    def entity_count(self) -> int: ...
    @property
    def slot_count(self) -> int: ...
    @property
    def cards_by_kind(self) -> Dict[str, int]: ...
    @property
    def enriched_frames(self) -> int: ...
    @property
    def last_enrichment(self) -> Optional[int]: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Schema
# ---------------------------------------------------------------------------

class PredicateSchema:
    """A registered predicate schema for memory card validation."""

    @property
    def id(self) -> str: ...
    @property
    def name(self) -> str: ...
    @property
    def description(self) -> Optional[str]: ...
    @property
    def domain(self) -> List[str]: ...
    @property
    def range(self) -> str: ...
    @property
    def range_entity_kind(self) -> Optional[str]: ...
    @property
    def range_enum_values(self) -> Optional[List[str]]: ...
    @property
    def cardinality(self) -> str: ...
    @property
    def inverse(self) -> Optional[str]: ...
    @property
    def builtin(self) -> bool: ...
    def __repr__(self) -> str: ...

class ValidationResult:
    """Result of :meth:`Memvid.validate_card`."""

    @property
    def valid(self) -> bool: ...
    @property
    def error(self) -> Optional[str]: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Logic mesh
# ---------------------------------------------------------------------------

class MeshNode:
    """An entity node in the logic mesh."""

    @property
    def id(self) -> int: ...
    @property
    def canonical_name(self) -> str: ...
    @property
    def display_name(self) -> str: ...
    @property
    def kind(self) -> str: ...
    @property
    def confidence(self) -> float: ...
    @property
    def frame_ids(self) -> List[int]: ...
    def __repr__(self) -> str: ...

class FollowResult:
    """A node reached by :meth:`Memvid.follow`."""

    @property
    def node(self) -> str: ...
    @property
    def kind(self) -> str: ...
    @property
    def confidence(self) -> float: ...
    @property
    def frame_ids(self) -> List[int]: ...
    @property
    def path_length(self) -> int: ...
    def __repr__(self) -> str: ...

class LogicMeshStats:
    """Statistics about the logic mesh."""

    @property
    def node_count(self) -> int: ...
    @property
    def edge_count(self) -> int: ...
    @property
    def entity_kinds(self) -> Dict[str, int]: ...
    @property
    def link_types(self) -> Dict[str, int]: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Sketches
# ---------------------------------------------------------------------------

class SketchTrackStats:
    """Statistics about the sketch track."""

    @property
    def variant(self) -> str: ...
    @property
    def entry_count(self) -> int: ...
    @property
    def size_bytes(self) -> int: ...
    @property
    def short_text_count(self) -> int: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Enrichment
# ---------------------------------------------------------------------------

class EnrichmentWorkerStats:
    """Statistics for a running enrichment worker."""

    @property
    def processed(self) -> int: ...
    @property
    def pending(self) -> int: ...
    @property
    def errors(self) -> int: ...
    @property
    def embeddings_generated(self) -> int: ...
    @property
    def re_extractions(self) -> int: ...
    @property
    def is_running(self) -> bool: ...

class EnrichmentHandle:
    """Handle to a running background enrichment worker."""

    @property
    def is_running(self) -> bool:
        """Whether the worker is still running."""
        ...

    def stats(self) -> EnrichmentWorkerStats:
        """Return current worker statistics."""
        ...

    def stop(self) -> None:
        """Stop the worker gracefully."""
        ...

# ---------------------------------------------------------------------------
# Document readers
# ---------------------------------------------------------------------------

class ReaderDiagnostics:
    """Diagnostics from a document reader extraction."""

    @property
    def warnings(self) -> List[str]: ...
    @property
    def fallback(self) -> bool: ...
    @property
    def duration_ms(self) -> Optional[int]: ...
    @property
    def pages_processed(self) -> Optional[int]: ...
    def __repr__(self) -> str: ...

class ReaderOutput:
    """Output from :meth:`ReaderRegistry.extract`."""

    @property
    def text(self) -> Optional[str]: ...
    @property
    def metadata(self) -> dict: ...
    @property
    def format(self) -> Optional[str]: ...
    @property
    def reader_name(self) -> str: ...
    @property
    def diagnostics(self) -> ReaderDiagnostics: ...
    def __repr__(self) -> str: ...

class ReaderRegistry:
    """Registry of built-in document readers (PDF, DOCX, XLSX, etc.)."""

    def __init__(self) -> None:
        """Create a registry with all built-in readers."""
        ...

    def extract(
        self,
        data: bytes,
        *,
        filename: Optional[str] = None,
        mime_type: Optional[str] = None,
    ) -> ReaderOutput:
        """Extract text and metadata from *data*."""
        ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Maintenance / verification
# ---------------------------------------------------------------------------

class VerificationCheck:
    """A single check within a verification report."""

    @property
    def name(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def details(self) -> Optional[str]: ...
    def __repr__(self) -> str: ...

class VerificationReport:
    """Result of :meth:`Memvid.verify`."""

    @property
    def file_path(self) -> str: ...
    @property
    def overall_status(self) -> str: ...
    @property
    def checks(self) -> List[VerificationCheck]: ...
    def __repr__(self) -> str: ...

class DoctorFinding:
    """A finding from doctor diagnostics."""

    @property
    def code(self) -> str: ...
    @property
    def severity(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def detail(self) -> Optional[str]: ...
    def __repr__(self) -> str: ...

class DoctorActionReport:
    """Report for a single doctor action."""

    @property
    def action(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def detail(self) -> Optional[str]: ...

class DoctorPhaseReport:
    """Report for a single doctor phase."""

    @property
    def phase(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def duration_ms(self) -> Optional[int]: ...
    @property
    def actions(self) -> List[DoctorActionReport]: ...

class DoctorMetrics:
    """Aggregate metrics from a doctor run."""

    @property
    def total_duration_ms(self) -> int: ...
    @property
    def actions_completed(self) -> int: ...
    @property
    def actions_skipped(self) -> int: ...
    @property
    def phase_durations(self) -> List[Tuple[str, int]]: ...

class DoctorActionPlan:
    """A planned doctor action."""

    @property
    def action(self) -> str: ...
    @property
    def required(self) -> bool: ...
    @property
    def note(self) -> Optional[str]: ...
    @property
    def reasons(self) -> List[str]: ...

class DoctorPhasePlan:
    """A planned doctor phase."""

    @property
    def phase(self) -> str: ...
    @property
    def actions(self) -> List[DoctorActionPlan]: ...

class DoctorPlan:
    """A repair plan generated by :meth:`Memvid.doctor_plan`."""

    @property
    def version(self) -> int: ...
    @property
    def file_path(self) -> str: ...
    @property
    def findings(self) -> List[DoctorFinding]: ...
    @property
    def phases(self) -> List[DoctorPhasePlan]: ...
    def __repr__(self) -> str: ...

class DoctorReport:
    """Result of :meth:`Memvid.doctor` or :meth:`Memvid.doctor_apply`."""

    @property
    def status(self) -> str: ...
    @property
    def plan(self) -> DoctorPlan: ...
    @property
    def phases(self) -> List[DoctorPhaseReport]: ...
    @property
    def findings(self) -> List[DoctorFinding]: ...
    @property
    def metrics(self) -> DoctorMetrics: ...
    @property
    def verification(self) -> Optional[VerificationReport]: ...
    def __repr__(self) -> str: ...

class Stats:
    """Comprehensive file statistics from :meth:`Memvid.stats`."""

    @property
    def frame_count(self) -> int: ...
    @property
    def size_bytes(self) -> int: ...
    @property
    def tier(self) -> str: ...
    @property
    def has_lex_index(self) -> bool: ...
    @property
    def has_vec_index(self) -> bool: ...
    @property
    def has_clip_index(self) -> bool: ...
    @property
    def has_time_index(self) -> bool: ...
    @property
    def seq_no(self) -> Optional[int]: ...
    @property
    def capacity_bytes(self) -> int: ...
    @property
    def active_frame_count(self) -> int: ...
    @property
    def payload_bytes(self) -> int: ...
    @property
    def logical_bytes(self) -> int: ...
    @property
    def saved_bytes(self) -> int: ...
    @property
    def compression_ratio_percent(self) -> float: ...
    @property
    def savings_percent(self) -> float: ...
    @property
    def storage_utilisation_percent(self) -> float: ...
    @property
    def remaining_capacity_bytes(self) -> int: ...
    @property
    def average_frame_payload_bytes(self) -> int: ...
    @property
    def average_frame_logical_bytes(self) -> int: ...
    @property
    def wal_bytes(self) -> int: ...
    @property
    def lex_index_bytes(self) -> int: ...
    @property
    def vec_index_bytes(self) -> int: ...
    @property
    def time_index_bytes(self) -> int: ...
    @property
    def vector_count(self) -> int: ...
    @property
    def clip_image_count(self) -> int: ...
    def __repr__(self) -> str: ...
