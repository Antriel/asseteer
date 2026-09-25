import { describe, expect, it } from 'vitest';
import { type Asset, getAssetRelativeDirectory } from './index';

function asset(partial: Partial<Asset>): Asset {
  return {
    id: 1,
    filename: 'a.wav',
    folder_id: 1,
    rel_path: '',
    zip_file: null,
    zip_entry: null,
    folder_path: 'C:/Sounds/library',
    asset_type: 'audio',
    format: 'wav',
    file_size: 0,
    width: null,
    height: null,
    duration_ms: null,
    sample_rate: null,
    channels: null,
    created_at: 0,
    modified_at: 0,
    ...partial,
  };
}

describe('getAssetRelativeDirectory', () => {
  it('is just the source folder name for a file at its root', () => {
    expect(getAssetRelativeDirectory(asset({}))).toBe('library');
  });

  it('prefixes the relative path with the source folder name', () => {
    expect(getAssetRelativeDirectory(asset({ rel_path: 'Weapons/Guns' }))).toBe(
      'library/Weapons/Guns',
    );
  });

  it('copes with backslashes and a trailing separator on the source folder', () => {
    expect(getAssetRelativeDirectory(asset({ folder_path: 'D:\\SFX\\Pack\\' }))).toBe('Pack');
  });

  it('includes the zip and the directory inside it, not the entry filename', () => {
    const a = asset({
      rel_path: 'Packs',
      zip_file: 'Retro.zip',
      zip_entry: 'Extras/bonus.zip/retro_powerup.wav',
    });
    expect(getAssetRelativeDirectory(a)).toBe('library/Packs/Retro.zip/Extras/bonus.zip');
  });

  it('ends at the zip for an entry at the zip root', () => {
    const a = asset({ zip_file: 'Retro.zip', zip_entry: 'coin.wav' });
    expect(getAssetRelativeDirectory(a)).toBe('library/Retro.zip');
  });
});
