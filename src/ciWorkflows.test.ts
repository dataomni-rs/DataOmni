import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

type Scopes = {
  appChanged: boolean;
  siteChanged: boolean;
};

function extractScopeLoop(ciYaml: string): string {
  const match = ciYaml.match(
    /(app_changed=false[\s\S]*?echo "site_changed=\$\{site_changed\}" >> "\$GITHUB_OUTPUT")/,
  );
  if (!match) {
    throw new Error('Could not locate Detect changed scopes loop in .github/workflows/ci.yml');
  }
  return match[1]
    .split('\n')
    .map((line) => line.replace(/^\s{10}/, ''))
    .join('\n')
    .replace('"${files[@]}"', '"$@"')
    .replace(/ >> "\$GITHUB_OUTPUT"/g, '');
}

function evaluateCiScopes(files: string[], ciYaml = readFileSync('.github/workflows/ci.yml', 'utf8')): Scopes {
  const script = `set -eu\n${extractScopeLoop(ciYaml)}\n`;
  const stdout = execFileSync('sh', ['-c', script, 'sh', ...files], { encoding: 'utf8' });
  const entries = Object.fromEntries(
    stdout
      .trim()
      .split('\n')
      .map((line) => line.split('=')),
  );
  return {
    appChanged: entries.app_changed === 'true',
    siteChanged: entries.site_changed === 'true',
  };
}

describe.skipIf(process.platform === 'win32')('CI 变更范围过滤与工作流门禁', () => {
  it('判据能区分免编译辅助路径与应用代码路径', () => {
    expect(
      evaluateCiScopes([
        'packaging/homebrew/Casks/dataomni.rb',
        'packaging/homebrew/README.md',
        'scripts/homebrew-cask.sh',
        'scripts/winget-manifest.sh',
        '.github/workflows/homebrew-tap.yml',
        '.github/workflows/windows-install.yml',
        'README.en.md',
        'TODOs.md',
        'rfcs/roadmap-1.3.md',
      ]),
    ).toEqual({
      appChanged: false,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['site/src/docs.rs'])).toEqual({
      appChanged: false,
      siteChanged: true,
    });

    expect(evaluateCiScopes(['README.md'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['scripts/install-macos.sh'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['scripts/schema-diff-verify.sh'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['src-tauri/src/lib.rs'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['.github/workflows/ci.yml'])).toEqual({
      appChanged: true,
      siteChanged: true,
    });
  });
});
