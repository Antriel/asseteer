import { DatabaseSync } from 'node:sqlite';
import { beforeAll, describe, expect, it } from 'vitest';
import {
  buildFtsFilter,
  parseSearchQuery,
  splitAlternatives,
  joinAlternatives,
  semanticAlternatives,
  splitForEditing,
  splitTerms,
  suggestAlternatives,
  type SearchColumn,
} from './searchQuery';

describe('parseSearchQuery', () => {
  it('spaces separate terms, commas separate alternatives', () => {
    expect(parseSearchQuery('gun shot, explosion')).toEqual([['gun', 'shot'], ['explosion']]);
  });

  it('drops empty alternatives and extra whitespace', () => {
    expect(parseSearchQuery(' ,gun ,,  , explosion  ,')).toEqual([['gun'], ['explosion']]);
    expect(parseSearchQuery(' , ')).toEqual([]);
  });

  it('keeps punctuation inside terms', () => {
    expect(parseSearchQuery('sci-fi (1) a:b *x^')).toEqual([['sci-fi', '(1)', 'a:b', '*x^']]);
  });

  it('quotes keep spaces and commas literal', () => {
    expect(parseSearchQuery('"door open", "a, b" c')).toEqual([['door open'], ['a, b', 'c']]);
  });

  it('an unclosed quote runs to the end', () => {
    expect(parseSearchQuery('laser "big, bang')).toEqual([['laser', 'big, bang']]);
  });
});

describe('splitAlternatives', () => {
  it('round-trips the input', () => {
    const text = 'gun shot, "a, b" ,, expl';
    expect(splitAlternatives(text)).toEqual(['gun shot', ' "a, b" ', '', ' expl']);
    expect(splitAlternatives(text).join(',')).toBe(text);
  });
});

describe('splitForEditing', () => {
  it('commits every alternative but the last', () => {
    expect(splitForEditing('gun shot, ,explosion,  la')).toEqual({
      committed: ['gun shot', 'explosion'],
      editing: 'la',
    });
    expect(splitForEditing(' gun ')).toEqual({ committed: [], editing: ' gun ' });
    expect(splitForEditing(',')).toEqual({ committed: [], editing: '' });
    expect(splitForEditing('"a, b"')).toEqual({ committed: [], editing: '"a, b"' });
  });

  it('round-trips through joinAlternatives', () => {
    for (const text of ['gun, explosion, la', 'gun, ', 'gun', '']) {
      const { committed, editing } = splitForEditing(text);
      expect(joinAlternatives(committed, editing)).toBe(text);
    }
    expect(joinAlternatives([], 'x')).toBe('x');
  });
});

describe('semanticAlternatives', () => {
  it('each alternative is one phrase; quotes only protect commas', () => {
    expect(semanticAlternatives('footsteps on  wood, "heavy, metallic impact" ,, ')).toEqual([
      'footsteps on wood',
      'heavy, metallic impact',
    ]);
    expect(semanticAlternatives(' , ')).toEqual([]);
  });
});

describe('splitTerms', () => {
  it('splits at quotes as well as whitespace', () => {
    expect(splitTerms('sci"fi x"y')).toEqual(['sci', 'fi x', 'y']);
  });
});

describe('suggestAlternatives', () => {
  it('turns words into alternatives, dropping "or"', () => {
    expect(suggestAlternatives('gun explosion')).toBe('gun, explosion');
    expect(suggestAlternatives('gun OR explosion | laser')).toBe('gun, explosion, laser');
    expect(suggestAlternatives('"door open" laser')).toBe('"door open", laser');
  });

  it('nothing to suggest for one word or a query that already has alternatives', () => {
    expect(suggestAlternatives('gun')).toBeNull();
    expect(suggestAlternatives('gun or')).toBeNull();
    expect(suggestAlternatives('gun shot, laser')).toBeNull();
  });
});

describe('buildFtsFilter', () => {
  it('returns null for blank input', () => {
    expect(buildFtsFilter('  , ', 'anywhere')).toBeNull();
  });

  it('quotes every term so user text is never FTS syntax', () => {
    const f = buildFtsFilter('say "hi', 'anywhere')!;
    expect(f.where.params).toEqual(['"say"', '"say"*', '"hi"*']);
  });

  it('targets a column', () => {
    const f = buildFtsFilter('gun', 'filename')!;
    expect(f.where.params).toEqual(['filename : "gun"', 'filename : "gun"*']);
  });

  it('only ranks when there are alternatives', () => {
    expect(buildFtsFilter('gun shot', 'anywhere')!.rank).toBeNull();
    expect(buildFtsFilter('gun, shot', 'anywhere')!.rank).not.toBeNull();
  });
});

// Real FTS5 tables with the app's tokenizers (src-tauri/src/database/schema.rs)
describe('buildFtsFilter against FTS5', () => {
  const db = new DatabaseSync(':memory:');
  const files: [string, string][] = [
    ['gun_shot_01.wav', 'Weapons'],
    ['gun_shot_02.wav', 'Weapons'],
    ['reload_pistol.wav', 'Weapons'],
    ['explosion_big.wav', 'Explosions'],
    ['explosion_small.wav', 'Explosions'],
    ['laser_blast.wav', 'Sci-Fi'],
    ['sci-fi_door_open.wav', 'Sci-Fi'],
    ['big gun, loud.wav', 'Misc'],
    ['café_ambience.wav', 'Ambience'],
  ];

  beforeAll(() => {
    db.exec(`
      CREATE TABLE assets (id INTEGER PRIMARY KEY, filename TEXT, searchable_path TEXT);
      CREATE VIRTUAL TABLE assets_fts_sub USING fts5(filename, searchable_path, tokenize='trigram');
      CREATE VIRTUAL TABLE assets_fts_word USING fts5(filename, searchable_path,
        tokenize="unicode61 separators '_-.'");
    `);
    files.forEach(([filename, path], i) => {
      for (const table of ['assets', 'assets_fts_sub', 'assets_fts_word']) {
        db.prepare(`INSERT INTO ${table} (rowid, filename, searchable_path) VALUES (?, ?, ?)`).run(
          i + 1,
          filename,
          path,
        );
      }
    });
  });

  function search(text: string, column: SearchColumn = 'anywhere'): string[] {
    const f = buildFtsFilter(text, column)!;
    const order = f.rank ? `${f.rank.sql} DESC, ` : '';
    const sql = `SELECT filename FROM assets WHERE ${f.where.sql}
      ORDER BY ${order}filename COLLATE NOCASE`;
    const rows = db.prepare(sql).all(...f.where.params, ...(f.rank?.params ?? []));
    return rows.map((r) => r.filename as string);
  }

  it('AND within an alternative, across columns', () => {
    expect(search('gun 02')).toEqual(['gun_shot_02.wav']);
    expect(search('pistol weapons')).toEqual(['reload_pistol.wav']);
  });

  it('OR across alternatives', () => {
    expect(search('pistol, laser')).toEqual(['laser_blast.wav', 'reload_pistol.wav']);
  });

  it('mixes short (word-prefix) and long (substring) terms in one alternative', () => {
    // "xplo" only matches as a substring, "sm" only as a word prefix
    expect(search('xplo sm')).toEqual(['explosion_small.wav']);
  });

  it('ranks rows matching more alternatives first', () => {
    expect(search('explosion, big')).toEqual([
      'explosion_big.wav',
      'big gun, loud.wav',
      'explosion_small.wav',
    ]);
  });

  it('punctuation is plain text', () => {
    expect(search('sci-fi door')).toEqual(['sci-fi_door_open.wav']);
    expect(search('(1)')).toEqual([]);
    expect(search('"gun, loud"')).toEqual(['big gun, loud.wav']);
  });

  it('matches without diacritics through the word table', () => {
    expect(search('cafe')).toEqual(['café_ambience.wav']);
  });

  it('respects column targeting', () => {
    expect(search('sci', 'filename')).toEqual(['sci-fi_door_open.wav']);
    expect(search('sci', 'path')).toEqual(['laser_blast.wav', 'sci-fi_door_open.wav']);
  });

  it('a term with no word characters matches nothing instead of failing', () => {
    expect(search('_')).toEqual([]);
    expect(search('_, laser')).toEqual(['laser_blast.wav']);
  });
});
