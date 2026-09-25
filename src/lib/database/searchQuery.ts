/**
 * Search box syntax → FTS5 SQL.
 *
 * Syntax: spaces separate words that must ALL match; commas separate alternatives, ANY of
 * which may match. `gun shot, explosion` = (gun AND shot) OR explosion. Double quotes keep
 * spaces and commas literal: `"door open"`. Nothing the user types is ever FTS5 syntax —
 * every term is quoted — so `sci-fi`, `(1)` or `a:b` are plain text.
 *
 * Each term matches as a substring (trigram table) or a word prefix (word table); terms
 * under 3 characters only as a word prefix, since trigram can't match them.
 */

/** Which FTS column(s) to search */
export type SearchColumn = 'anywhere' | 'filename' | 'path';

/**
 * Split the raw input at commas outside quotes. Returns the raw (untrimmed) text of each
 * alternative, including empty ones, so `groups.join(',')` gives back the input.
 */
export function splitAlternatives(text: string): string[] {
  const groups: string[] = [];
  let start = 0;
  let inQuote = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') inQuote = !inQuote;
    else if (c === ',' && !inQuote) {
      groups.push(text.slice(start, i));
      start = i + 1;
    }
  }
  groups.push(text.slice(start));
  return groups;
}

/** Split one alternative into terms: whitespace separates, quotes group. */
export function splitTerms(group: string): string[] {
  const terms: string[] = [];
  let current = '';
  let inQuote = false;
  const flush = () => {
    const term = current.trim();
    if (term) terms.push(term);
    current = '';
  };
  for (const c of group) {
    if (c === '"') {
      flush();
      inQuote = !inQuote;
    } else if (!inQuote && /\s/.test(c)) {
      flush();
    } else {
      current += c;
    }
  }
  flush();
  return terms;
}

/** Parse the input into alternatives (OR) of terms (AND). Empty alternatives are dropped. */
export function parseSearchQuery(text: string): string[][] {
  return splitAlternatives(text)
    .map(splitTerms)
    .filter((terms) => terms.length > 0);
}

/** Minimum length for the trigram (substring) table; shorter terms only match word prefixes */
const TRIGRAM_MIN = 3;

function ftsString(term: string): string {
  return `"${term.replace(/"/g, '""')}"`;
}

function columnPrefix(column: SearchColumn): string {
  return column === 'filename' ? 'filename : ' : column === 'path' ? 'searchable_path : ' : '';
}

export interface SqlFragment {
  sql: string;
  params: string[];
}

/** SELECT of the rowids (as `id`) matching one term */
function termSelect(term: string, column: SearchColumn): SqlFragment {
  const col = columnPrefix(column);
  const word = {
    sql: 'SELECT rowid AS id FROM assets_fts_word WHERE assets_fts_word MATCH ?',
    params: [`${col}${ftsString(term)}*`],
  };
  if ([...term].length < TRIGRAM_MIN) return word;
  return {
    sql: `SELECT rowid AS id FROM assets_fts_sub WHERE assets_fts_sub MATCH ? UNION ${word.sql}`,
    params: [`${col}${ftsString(term)}`, ...word.params],
  };
}

/** Combine SELECTs of `id` with a compound operator, each nested so precedence can't leak */
function combine(parts: SqlFragment[], op: 'INTERSECT' | 'UNION'): SqlFragment {
  if (parts.length === 1) return parts[0];
  return {
    sql: parts.map((p) => `SELECT id FROM (${p.sql})`).join(` ${op} `),
    params: parts.flatMap((p) => p.params),
  };
}

/** SELECT of the rowids matching one alternative (all its terms) */
function alternativeSelect(terms: string[], column: SearchColumn): SqlFragment {
  return combine(
    terms.map((t) => termSelect(t, column)),
    'INTERSECT',
  );
}

export interface FtsFilter {
  /** WHERE condition restricting `assets.id` to matches */
  where: SqlFragment;
  /**
   * With several alternatives: an ORDER BY key (higher first) counting how many
   * alternatives each row matches, so rows matching more of them rank first.
   */
  rank: SqlFragment | null;
}

/** Build the FTS filter for the search box text, or null when there is nothing to search. */
export function buildFtsFilter(text: string, column: SearchColumn): FtsFilter | null {
  const alternatives = parseSearchQuery(text).map((terms) => alternativeSelect(terms, column));
  if (alternatives.length === 0) return null;

  const all = combine(alternatives, 'UNION');
  const where = { sql: `assets.id IN (${all.sql})`, params: all.params };
  if (alternatives.length === 1) return { where, rank: null };

  return {
    where,
    rank: {
      sql: alternatives.map((a) => `(assets.id IN (${a.sql}))`).join(' + '),
      params: alternatives.flatMap((a) => a.params),
    },
  };
}

/** Words people type meaning OR, dropped when suggesting the comma form */
const OR_WORDS = new Set(['or', '|', '||']);

/**
 * For a query of several words and no commas — which requires ALL of them — the same words
 * as alternatives: `gun or explosion` → `gun, explosion`. Null when not applicable.
 */
export function suggestAlternatives(text: string): string | null {
  const alternatives = parseSearchQuery(text);
  if (alternatives.length !== 1) return null;
  const terms = alternatives[0].filter((t) => !OR_WORDS.has(t.toLowerCase()));
  if (terms.length < 2) return null;
  return terms.map((t) => (/[\s,]/.test(t) ? `"${t}"` : t)).join(', ');
}

/**
 * Split the input into committed alternatives (trimmed, blanks dropped) and the one still
 * being typed — the search box shows the former as chips and edits only the last.
 */
export function splitForEditing(text: string): { committed: string[]; editing: string } {
  const parts = splitAlternatives(text);
  const last = parts.pop()!;
  const committed = parts.map((p) => p.trim()).filter(Boolean);
  return { committed, editing: committed.length ? last.trimStart() : last };
}

/** Inverse of splitForEditing */
export function joinAlternatives(committed: string[], editing: string): string {
  return committed.length ? [...committed, editing].join(', ') : editing;
}

/**
 * Semantic (CLAP) search takes each alternative as one phrase: no AND, quotes only protect
 * commas — `"heavy, metallic impact", door creak` → two phrases.
 */
export function semanticAlternatives(text: string): string[] {
  return splitAlternatives(text)
    .map((alt) => alt.replace(/"/g, '').replace(/\s+/g, ' ').trim())
    .filter(Boolean);
}
