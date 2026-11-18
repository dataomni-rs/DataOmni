import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

type Scopes = {
  appChanged: boolean;
  siteChanged: boolean;
};

const NODE20_DEPRECATED_ACTIONS: ReadonlyArray<readonly [string, number]> = [
  ['actions/checkout', 7],
  ['actions/cache', 6],
  ['actions/upload-artifact', 7],
  ['actions/download-artifact', 8],
  ['actions/upload-pages-artifact', 5],
  ['actions/deploy-pages', 5],
];

function findOutdatedNode20Actions(workflowName: string, yamlText: string): string[] {
  const outdated: string[] = [];
  for (const [action, minMajor] of NODE20_DEPRECATED_ACTIONS) {
    const escaped = action.replace('/', '\\/');
    const regex = new RegExp(`uses:\\s*${escaped}@v(\\d+)`, 'g');
    for (const match of yamlText.matchAll(regex)) {
      const major = Number(match[1]);
      if (major < minMajor) {
        outdated.push(`${workflowName}: ${action}@v${major} (< v${minMajor})`);
      }
    }
  }
  return outdated;
}

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

  it('判据能识别仍停留在 Node 20 的旧版官方 Action', () => {
    const sample = [
      '- uses: actions/checkout@v4',
      '- uses: actions/cache@v4',
      '- uses: actions/upload-artifact@v4',
      '- uses: actions/download-artifact@v4',
      '- uses: actions/upload-pages-artifact@v3',
      '- uses: actions/deploy-pages@v4',
      '- uses: actions/checkout@v7',
    ].join('\n');

    expect(findOutdatedNode20Actions('sample.yml', sample)).toEqual([
      'sample.yml: actions/checkout@v4 (< v7)',
      'sample.yml: actions/cache@v4 (< v6)',
      'sample.yml: actions/upload-artifact@v4 (< v7)',
      'sample.yml: actions/download-artifact@v4 (< v8)',
      'sample.yml: actions/upload-pages-artifact@v3 (< v5)',
      'sample.yml: actions/deploy-pages@v4 (< v5)',
    ]);
  });

  it('所有 GitHub Actions 工作流均已升级至原生 Node 24 主版本', () => {
    const dir = '.github/workflows';
    const findings = readdirSync(dir)
      .filter((name) => name.endsWith('.yml'))
      .sort()
      .flatMap((name) => findOutdatedNode20Actions(name, readFileSync(join(dir, name), 'utf8')));

    expect(findings).toEqual([]);
  });
});
