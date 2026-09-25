declare const __APP_VERSION__: string;

interface Window {
  /** Dev builds only — installed by `$lib/harness/testHooks`. Check before use. */
  asseteerTest?: import('$lib/harness/testHooks').AsseteerTestHooks;
}
