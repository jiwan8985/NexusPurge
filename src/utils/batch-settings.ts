const KEYS = {
  maxConcurrentTransfers: "nexuspurge.maxConcurrentTransfers",
  fileCountWarn:          "nexuspurge.fileCountWarnThreshold",
  fileCountLimit:         "nexuspurge.fileCountLimitThreshold",
  purgeBatchSize:         "nexuspurge.purgeBatchSize",
  purgeWarnThreshold:     "nexuspurge.purgeWarnThreshold",
  largeSizeMb:            "nexuspurge.largeSizeMbThreshold",
} as const;

export const BATCH_DEFAULTS = {
  maxConcurrentTransfers: 4,
  fileCountWarn:          5_000,
  fileCountLimit:         10_000,
  purgeBatchSize:         1_000,
  purgeWarnThreshold:     1_000,
  largeSizeMb:            100,
} as const;

function readInt(key: string, fallback: number): number {
  const raw = window.localStorage.getItem(key);
  const n = raw !== null ? parseInt(raw, 10) : NaN;
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

export function readBatchSettings() {
  return {
    maxConcurrentTransfers: readInt(KEYS.maxConcurrentTransfers, BATCH_DEFAULTS.maxConcurrentTransfers),
    fileCountWarn:          readInt(KEYS.fileCountWarn,          BATCH_DEFAULTS.fileCountWarn),
    fileCountLimit:         readInt(KEYS.fileCountLimit,         BATCH_DEFAULTS.fileCountLimit),
    purgeBatchSize:         readInt(KEYS.purgeBatchSize,         BATCH_DEFAULTS.purgeBatchSize),
    purgeWarnThreshold:     readInt(KEYS.purgeWarnThreshold,     BATCH_DEFAULTS.purgeWarnThreshold),
    largeSizeMb:            readInt(KEYS.largeSizeMb,            BATCH_DEFAULTS.largeSizeMb),
  };
}

export function writeBatchSetting(key: keyof typeof KEYS, value: number): void {
  window.localStorage.setItem(KEYS[key], String(value));
}
