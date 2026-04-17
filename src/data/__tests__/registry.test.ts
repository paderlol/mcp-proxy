import { describe, it, expect } from "vitest";
import {
  filterEntries,
  REGISTRY_ENTRIES,
  type RegistryEntry,
} from "../registry";

// Sanity: the bundled registry data itself must stay well-formed.
describe("REGISTRY_ENTRIES invariants", () => {
  it("has entries in both regions", () => {
    const internationalCount = REGISTRY_ENTRIES.filter(
      (e) => e.region === "international",
    ).length;
    const chinaCount = REGISTRY_ENTRIES.filter((e) => e.region === "china").length;
    expect(internationalCount).toBeGreaterThan(0);
    expect(chinaCount).toBeGreaterThan(0);
  });

  it("uses unique IDs", () => {
    const ids = REGISTRY_ENTRIES.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("always sets a non-empty publisher", () => {
    for (const entry of REGISTRY_ENTRIES) {
      expect(entry.publisher.trim().length).toBeGreaterThan(0);
    }
  });

  it("ensures every env var spec has a name and description", () => {
    for (const entry of REGISTRY_ENTRIES) {
      for (const env of entry.envVars) {
        expect(env.name.trim().length).toBeGreaterThan(0);
        expect(env.description.trim().length).toBeGreaterThan(0);
      }
    }
  });
});

describe("filterEntries()", () => {
  it("returns only entries from the requested region", () => {
    const result = filterEntries("international", "");
    expect(result.length).toBeGreaterThan(0);
    for (const entry of result) {
      expect(entry.region).toBe("international");
    }
  });

  it("returns an empty list when the region has no matches", () => {
    // Empty query + china region yields the full china subset
    const chinaAll = filterEntries("china", "");
    expect(chinaAll.length).toBeGreaterThan(0);
    // A query that matches nothing
    const nothing = filterEntries("china", "zzz-definitely-nothing");
    expect(nothing).toEqual([]);
  });

  it("matches by name (case-insensitive)", () => {
    // "GitHub" exists in the fixture data
    const upper = filterEntries("international", "GITHUB");
    const lower = filterEntries("international", "github");
    expect(upper).toEqual(lower);
    expect(upper.length).toBeGreaterThan(0);
    expect(upper.every((e) => e.name.toLowerCase().includes("github"))).toBe(
      true,
    );
  });

  it("matches by publisher", () => {
    // Most international entries are published by Anthropic
    const results = filterEntries("international", "Anthropic");
    expect(results.length).toBeGreaterThan(0);
    expect(
      results.every((e) => e.publisher.toLowerCase().includes("anthropic")),
    ).toBe(true);
  });

  it("matches by tag", () => {
    // Chinese map servers are tagged "地图"
    const results = filterEntries("china", "地图");
    expect(results.length).toBeGreaterThan(0);
    for (const entry of results) {
      expect(entry.tags).toContain("地图");
    }
  });

  it("matches by id substring", () => {
    const results = filterEntries("international", "postgres");
    expect(results.map((e) => e.id)).toContain("postgres");
  });

  it("matches by description content", () => {
    const results = filterEntries("international", "knowledge graph");
    expect(results.map((e) => e.id)).toContain("memory");
  });

  it("does not cross regions — a china-only tag returns nothing in international", () => {
    const results = filterEntries("international", "地图");
    expect(results).toEqual([]);
  });

  it("trims and ignores whitespace-only queries", () => {
    const all = filterEntries("international", "");
    const whitespace = filterEntries("international", "   ");
    expect(whitespace.length).toBe(all.length);
  });

  it("preserves the type shape of returned entries", () => {
    const results = filterEntries("international", "github");
    const first: RegistryEntry | undefined = results[0];
    expect(first).toBeDefined();
    expect(first).toMatchObject({
      id: expect.any(String),
      name: expect.any(String),
      publisher: expect.any(String),
      region: "international",
      tags: expect.any(Array),
      command: expect.any(String),
      args: expect.any(Array),
      envVars: expect.any(Array),
      sourceUrl: expect.any(String),
    });
  });
});
