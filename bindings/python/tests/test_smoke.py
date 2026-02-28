"""Smoke tests for the memvid Python bindings.

These tests verify that the native extension loads correctly, that the
main classes and functions are importable, and that basic create/write/read
round-trips work end-to-end.

Run with::

    pytest bindings/python/tests/test_smoke.py -v
"""

from __future__ import annotations

import os
import tempfile

import pytest

import memvid


# ---------------------------------------------------------------------------
# Module-level
# ---------------------------------------------------------------------------


def test_version_returns_string():
    v = memvid.version()
    assert isinstance(v, str)
    assert len(v) > 0


def test_memvid_error_is_importable():
    assert issubclass(memvid.MemvidError, Exception)


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------


def test_create_and_close(tmp_path):
    path = str(tmp_path / "test.mv2")
    mv = memvid.Memvid.create(path)
    assert mv.path == path
    assert not mv.is_read_only
    mv.close()


def test_context_manager(tmp_path):
    path = str(tmp_path / "ctx.mv2")
    with memvid.Memvid.create(path) as mv:
        assert mv.path == path
    # After exiting the context manager, operations should raise
    with pytest.raises(memvid.MemvidError) as exc_info:
        mv.frame_count  # noqa: B018
    assert exc_info.value.code == "CLOSED"


def test_open_and_open_read_only(tmp_path):
    path = str(tmp_path / "reopen.mv2")
    with memvid.Memvid.create(path) as mv:
        mv.put_bytes(b"hello")
        mv.commit()

    with memvid.Memvid.open(path) as mv:
        assert not mv.is_read_only
        assert mv.frame_count >= 1

    with memvid.Memvid.open_read_only(path) as mv:
        assert mv.is_read_only


# ---------------------------------------------------------------------------
# Write / read round-trip
# ---------------------------------------------------------------------------


def test_put_bytes_and_read_back(tmp_path):
    path = str(tmp_path / "write.mv2")
    with memvid.Memvid.create(path) as mv:
        fid = mv.put_bytes(b"hello world")
        mv.commit()
        assert isinstance(fid, int)
        assert fid >= 0

        payload = mv.frame_canonical_payload(fid)
        assert payload == b"hello world"

        text = mv.frame_text_by_id(fid)
        assert "hello world" in text


def test_put_bytes_with_options(tmp_path):
    path = str(tmp_path / "opts.mv2")
    with memvid.Memvid.create(path) as mv:
        fid = mv.put_bytes_with_options(
            b"tagged content",
            uri="test://doc1",
            title="Test Doc",
            tags=["alpha", "beta"],
        )
        mv.commit()

        frame = mv.frame_by_id(fid)
        assert frame.uri == "test://doc1"
        assert frame.title == "Test Doc"
        assert "alpha" in frame.tags


def test_delete_frame(tmp_path):
    path = str(tmp_path / "delete.mv2")
    with memvid.Memvid.create(path) as mv:
        fid = mv.put_bytes(b"to be deleted")
        mv.commit()
        tombstone = mv.delete_frame(fid)
        mv.commit()
        assert isinstance(tombstone, int)


# ---------------------------------------------------------------------------
# Frame metadata
# ---------------------------------------------------------------------------


def test_frame_metadata(tmp_path):
    path = str(tmp_path / "meta.mv2")
    with memvid.Memvid.create(path) as mv:
        fid = mv.put_bytes(b"metadata test")
        mv.commit()

        frame = mv.frame_by_id(fid)
        assert frame.id == fid
        assert isinstance(frame.timestamp, int)
        assert isinstance(frame.role, str)
        assert isinstance(frame.canonical_encoding, str)
        assert isinstance(frame.extra_metadata, dict)


def test_frame_count_and_next_id(tmp_path):
    path = str(tmp_path / "count.mv2")
    with memvid.Memvid.create(path) as mv:
        assert mv.frame_count == 0
        before_id = mv.next_frame_id
        mv.put_bytes(b"one")
        mv.commit()
        assert mv.frame_count >= 1
        assert mv.next_frame_id > before_id


# ---------------------------------------------------------------------------
# Search
# ---------------------------------------------------------------------------


def test_search_basic(tmp_path):
    path = str(tmp_path / "search.mv2")
    with memvid.Memvid.create(path) as mv:
        mv.put_bytes(b"The quick brown fox jumps over the lazy dog")
        mv.commit()

        resp = mv.search("fox")
        assert isinstance(resp, memvid.SearchResponse)
        assert resp.query == "fox"
        assert isinstance(resp.hits, list)
        assert isinstance(resp.elapsed_ms, int)
        assert isinstance(resp.engine, str)


# ---------------------------------------------------------------------------
# Timeline
# ---------------------------------------------------------------------------


def test_timeline(tmp_path):
    path = str(tmp_path / "timeline.mv2")
    with memvid.Memvid.create(path) as mv:
        mv.put_bytes(b"event one")
        mv.put_bytes(b"event two")
        mv.commit()

        entries = mv.timeline()
        assert isinstance(entries, list)
        if entries:
            e = entries[0]
            assert isinstance(e, memvid.TimelineEntry)
            assert isinstance(e.frame_id, int)
            assert isinstance(e.timestamp, int)


# ---------------------------------------------------------------------------
# Memory cards
# ---------------------------------------------------------------------------


def test_memory_card_round_trip(tmp_path):
    path = str(tmp_path / "memory.mv2")
    with memvid.Memvid.create(path) as mv:
        cid = mv.put_memory_card(
            kind="fact",
            entity="Alice",
            slot="favorite_color",
            value="blue",
        )
        mv.commit()
        assert isinstance(cid, int)

        card = mv.get_current_memory("Alice", "favorite_color")
        assert card is not None
        assert card.value == "blue"
        assert card.kind == "fact"
        assert card.entity == "Alice"

        assert mv.memory_card_count >= 1

        stats = mv.memories_stats()
        assert isinstance(stats, memvid.MemoriesStats)
        assert stats.card_count >= 1


# ---------------------------------------------------------------------------
# Batch operations
# ---------------------------------------------------------------------------


def test_batch_mode(tmp_path):
    path = str(tmp_path / "batch.mv2")
    with memvid.Memvid.create(path) as mv:
        mv.begin_batch()
        for i in range(10):
            mv.put_bytes(f"item {i}".encode())
        mv.end_batch()
        assert mv.frame_count >= 10


# ---------------------------------------------------------------------------
# Verification / stats
# ---------------------------------------------------------------------------


def test_verify(tmp_path):
    path = str(tmp_path / "verify.mv2")
    with memvid.Memvid.create(path) as mv:
        mv.put_bytes(b"data")
        mv.commit()

    report = memvid.Memvid.verify(path)
    assert isinstance(report, memvid.VerificationReport)
    assert report.file_path == path
    assert isinstance(report.overall_status, str)
    assert isinstance(report.checks, list)


def test_stats(tmp_path):
    path = str(tmp_path / "stats.mv2")
    with memvid.Memvid.create(path) as mv:
        mv.put_bytes(b"stats data")
        mv.commit()

        s = mv.stats()
        assert isinstance(s, memvid.Stats)
        assert s.frame_count >= 1
        assert isinstance(s.size_bytes, int)
        assert isinstance(s.has_lex_index, bool)


# ---------------------------------------------------------------------------
# Reader registry
# ---------------------------------------------------------------------------


def test_reader_registry():
    reg = memvid.ReaderRegistry()
    out = reg.extract(b"Hello, plain text!", filename="test.txt")
    assert isinstance(out, memvid.ReaderOutput)
    assert isinstance(out.reader_name, str)
    assert isinstance(out.diagnostics, memvid.ReaderDiagnostics)


# ---------------------------------------------------------------------------
# Class existence checks (ensure all public types are importable)
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "cls_name",
    [
        "Memvid",
        "MemvidError",
        "Frame",
        "FrameContext",
        "BlobReader",
        "TimelineEntry",
        "SearchHit",
        "SearchResponse",
        "AskStats",
        "AskCitation",
        "AskContextFragment",
        "AskResponse",
        "MemoryCard",
        "MemoriesStats",
        "PredicateSchema",
        "ValidationResult",
        "MeshNode",
        "FollowResult",
        "LogicMeshStats",
        "SketchTrackStats",
        "EnrichmentWorkerStats",
        "EnrichmentHandle",
        "ReaderDiagnostics",
        "ReaderOutput",
        "ReaderRegistry",
        "VerificationCheck",
        "VerificationReport",
        "DoctorFinding",
        "DoctorActionReport",
        "DoctorPhaseReport",
        "DoctorMetrics",
        "DoctorActionPlan",
        "DoctorPhasePlan",
        "DoctorPlan",
        "DoctorReport",
        "Stats",
    ],
)
def test_class_exists(cls_name):
    assert hasattr(memvid, cls_name), f"memvid.{cls_name} not found"
