import { useState } from "react";
import styles from "./UploadOptionsModal.module.css";

export interface UploadOptions {
  contentTypeOverride: string;
  cacheControl: string;
  headers: Array<{ key: string; value: string }>;
  metadata: Array<{ key: string; value: string }>;
}

const CACHE_CONTROL_PRESETS = [
  { label: "1년 (불변 에셋)", value: "public, max-age=31536000, immutable" },
  { label: "1일", value: "public, max-age=86400" },
  { label: "1시간", value: "public, max-age=3600" },
  { label: "캐시 없음", value: "no-cache, no-store, must-revalidate" },
  { label: "재검증", value: "no-cache" },
];

const CONTENT_TYPE_PRESETS = [
  { label: "자동 감지", value: "" },
  { label: "HTML", value: "text/html; charset=utf-8" },
  { label: "JSON", value: "application/json" },
  { label: "JS", value: "application/javascript" },
  { label: "CSS", value: "text/css" },
  { label: "PNG", value: "image/png" },
  { label: "JPEG", value: "image/jpeg" },
  { label: "WebP", value: "image/webp" },
  { label: "바이너리", value: "application/octet-stream" },
];

export const DEFAULT_UPLOAD_OPTIONS: UploadOptions = {
  contentTypeOverride: "",
  cacheControl: "",
  headers: [],
  metadata: [],
};

interface KVRowProps {
  row: { key: string; value: string };
  onChange: (row: { key: string; value: string }) => void;
  onRemove: () => void;
  keyPlaceholder: string;
  valuePlaceholder: string;
}

function KVRow({ row, onChange, onRemove, keyPlaceholder, valuePlaceholder }: KVRowProps) {
  return (
    <div className={styles.kvRow}>
      <input
        className={styles.kvInput}
        value={row.key}
        onChange={(e) => onChange({ ...row, key: e.target.value })}
        placeholder={keyPlaceholder}
      />
      <input
        className={styles.kvInput}
        value={row.value}
        onChange={(e) => onChange({ ...row, value: e.target.value })}
        placeholder={valuePlaceholder}
      />
      <button type="button" className={styles.kvRemove} onClick={onRemove} title="삭제">×</button>
    </div>
  );
}

interface Props {
  initial: UploadOptions;
  fileCount: number;
  onConfirm: (opts: UploadOptions) => void;
  onCancel: () => void;
}

export default function UploadOptionsModal({ initial, fileCount, onConfirm, onCancel }: Props) {
  const [opts, setOpts] = useState<UploadOptions>(initial);

  const set = <K extends keyof UploadOptions>(key: K, value: UploadOptions[K]) =>
    setOpts((prev) => ({ ...prev, [key]: value }));

  const updateHeader = (i: number, row: { key: string; value: string }) =>
    set("headers", opts.headers.map((h, idx) => (idx === i ? row : h)));

  const updateMeta = (i: number, row: { key: string; value: string }) =>
    set("metadata", opts.metadata.map((m, idx) => (idx === i ? row : m)));

  return (
    <div className={styles.overlay}>
      <div className={styles.dialog}>
        <div className={styles.header}>
          <span className={styles.title}>업로드 옵션</span>
          <span className={styles.subtitle}>{fileCount}개 파일</span>
        </div>

        <div className={styles.body}>
          {/* Content-Type */}
          <section className={styles.section}>
            <label className={styles.label}>Content-Type</label>
            <input
              className={styles.input}
              value={opts.contentTypeOverride}
              onChange={(e) => set("contentTypeOverride", e.target.value)}
              placeholder="비워두면 확장자 기반 자동 감지"
            />
            <div className={styles.presets}>
              {CONTENT_TYPE_PRESETS.map((p) => (
                <button
                  key={p.label}
                  type="button"
                  className={`${styles.preset} ${opts.contentTypeOverride === p.value ? styles.presetActive : ""}`}
                  onClick={() => set("contentTypeOverride", p.value)}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </section>

          {/* Cache-Control */}
          <section className={styles.section}>
            <label className={styles.label}>Cache-Control</label>
            <input
              className={styles.input}
              value={opts.cacheControl}
              onChange={(e) => set("cacheControl", e.target.value)}
              placeholder="비워두면 프로필 기본값 사용"
            />
            <div className={styles.presets}>
              {CACHE_CONTROL_PRESETS.map((p) => (
                <button
                  key={p.value}
                  type="button"
                  className={`${styles.preset} ${opts.cacheControl === p.value ? styles.presetActive : ""}`}
                  onClick={() => set("cacheControl", p.value)}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </section>

          {/* Custom Headers */}
          <section className={styles.section}>
            <div className={styles.sectionHeader}>
              <label className={styles.label}>커스텀 HTTP 헤더</label>
              <button
                type="button"
                className={styles.addBtn}
                onClick={() => set("headers", [...opts.headers, { key: "", value: "" }])}
              >
                + 추가
              </button>
            </div>
            {opts.headers.length === 0 && (
              <p className={styles.emptyHint}>헤더가 없습니다.</p>
            )}
            {opts.headers.map((h, i) => (
              <KVRow
                key={i}
                row={h}
                onChange={(row) => updateHeader(i, row)}
                onRemove={() => set("headers", opts.headers.filter((_, idx) => idx !== i))}
                keyPlaceholder="헤더 이름"
                valuePlaceholder="값"
              />
            ))}
          </section>

          {/* S3 Metadata */}
          <section className={styles.section}>
            <div className={styles.sectionHeader}>
              <label className={styles.label}>S3 Metadata (x-amz-meta-*)</label>
              <button
                type="button"
                className={styles.addBtn}
                onClick={() => set("metadata", [...opts.metadata, { key: "", value: "" }])}
              >
                + 추가
              </button>
            </div>
            {opts.metadata.length === 0 && (
              <p className={styles.emptyHint}>메타데이터가 없습니다.</p>
            )}
            {opts.metadata.map((m, i) => (
              <KVRow
                key={i}
                row={m}
                onChange={(row) => updateMeta(i, row)}
                onRemove={() => set("metadata", opts.metadata.filter((_, idx) => idx !== i))}
                keyPlaceholder="키 (x-amz-meta- 제외)"
                valuePlaceholder="값"
              />
            ))}
          </section>
        </div>

        <div className={styles.footer}>
          <button
            type="button"
            className={styles.btnReset}
            onClick={() => setOpts(DEFAULT_UPLOAD_OPTIONS)}
          >
            초기화
          </button>
          <div className={styles.footerRight}>
            <button type="button" className={styles.btnCancel} onClick={onCancel}>
              취소
            </button>
            <button type="button" className={styles.btnConfirm} onClick={() => onConfirm(opts)}>
              업로드 시작
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
