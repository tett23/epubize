import { expect } from 'jsr:@std/expect@^1';
import { describe, it } from 'jsr:@std/testing@^1/bdd';
import { isAdrPath, onlyStatusLineChanged, resultOfToolCall } from '../scripts/adr-guard.ts';

const ADR = '# ADR 0001: 決定\n\nステータス: 採択\n\n## 文脈\n\n本文\n';

describe('isAdrPath', () => {
  it('matches markdown files directly under docs/adr', () => {
    expect(isAdrPath('docs/adr/0001-x.md')).toBe(true);
    expect(isAdrPath('docs/adr/sub/0001-x.md')).toBe(false);
    expect(isAdrPath('docs/adr/notes.txt')).toBe(false);
    expect(isAdrPath('README.md')).toBe(false);
  });
});

describe('onlyStatusLineChanged', () => {
  it('allows changing only the status line', () => {
    const after = ADR.replace('ステータス: 採択', 'ステータス: 置換（ADR 0009）');
    expect(onlyStatusLineChanged(ADR, after)).toBe(true);
  });

  it('rejects body changes', () => {
    expect(onlyStatusLineChanged(ADR, ADR.replace('本文', '書き換えた本文'))).toBe(false);
  });

  it('rejects body changes made together with a status change', () => {
    const after = ADR.replace('ステータス: 採択', 'ステータス: 廃止').replace('本文', '別');
    expect(onlyStatusLineChanged(ADR, after)).toBe(false);
  });

  it('rejects adding, removing, or moving a status line', () => {
    expect(onlyStatusLineChanged(ADR, `${ADR}ステータス: 追加\n`)).toBe(false);
    expect(onlyStatusLineChanged(ADR, ADR.replace('ステータス: 採択\n', ''))).toBe(false);
  });
});

describe('resultOfToolCall', () => {
  it('computes Edit, MultiEdit, and Write results', () => {
    expect(resultOfToolCall('Edit', { old_string: '本文', new_string: 'X' }, ADR)).toContain('X\n');
    expect(
      resultOfToolCall(
        'MultiEdit',
        { edits: [{ old_string: '採択', new_string: '廃止' }, { old_string: '本文', new_string: 'Y' }] },
        ADR,
      ),
    ).toContain('ステータス: 廃止');
    expect(resultOfToolCall('Write', { content: 'new' }, ADR)).toBe('new');
  });

  it('returns undefined when the edit cannot apply', () => {
    expect(resultOfToolCall('Edit', { old_string: 'missing', new_string: 'x' }, ADR)).toBeUndefined();
  });
});
