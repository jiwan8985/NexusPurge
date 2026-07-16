import { useState } from "react";
import { createPortal } from "react-dom";
import type { PurgeExecutionResult, CdnProvider } from "../../types";
import styles from "./PurgeResultDialog.module.css";

// CDN 제공자별 정보
const CDN_INFO: Record<CdnProvider, { name: string; invalidationLabel?: string }> = {
  cloudfront: { name: "AWS CloudFront",  invalidationLabel: "Invalidation ID" },
  akamai:     { name: "Akamai" },
  lguplus:    { name: "LG U+ CDN",       invalidationLabel: "Transaction ID" },
  hyosung:    { name: "Hyosung ITX CDN", invalidationLabel: "Transaction ID" },
  kt:         { name: "KT CDN",          invalidationLabel: "Transaction ID" },
};

function fmtTime(iso: string) {
  return new Date(iso).toLocaleTimeString("ko-KR", {
    hour: "2-digit", minute: "2-digit", second: "2-digit",
  });
}

function elapsedSec(start: string, end: string) {
  return ((new Date(end).getTime() - new Date(start).getTime()) / 1000).toFixed(1);
}

interface Props {
  results: PurgeExecutionResult[];
  onClose: () => void;
}

export default function PurgeResultDialog({ results, onClose }: Props) {
  const [showPaths, setShowPaths] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);

  const result = results[Math.min(activeIndex, results.length - 1)];
  if (!result) return null;

  const isMulti = results.length > 1;
  const overallFailed = results.reduce((sum, r) => sum + r.failedCount, 0);
  const overallSuccess = results.reduce((sum, r) => sum + r.successCount, 0);

  const cdnInfo = CDN_INFO[result.provider];
  const isAllSuccess = result.failedCount === 0;
  const isAllFailed  = result.successCount === 0;

  const allInvalidationIds = result.batches
    .filter((b) => b.invalidationId)
    .map((b) => b.invalidationId!);

  const allPaths = result.batches.flatMap((b) => b.paths);
  const failedBatches = result.batches.filter((b) => !b.success);
  const elapsed = elapsedSec(result.startedAt, result.finishedAt);

  return createPortal(
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>

        {/* 헤더 */}
        <div className={styles.header}>
          <span className={styles.title}>
            CDN Purge 결과{isMulti && ` (${results.length}개 CDN)`}
          </span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

        {isMulti && (
          <div className={styles.providerTabs}>
            {results.map((r, i) => {
              const dotClass = r.failedCount === 0 ? "ok" : r.successCount === 0 ? "fail" : "partial";
              return (
                <button
                  key={r.provider}
                  className={`${styles.providerTab} ${i === activeIndex ? styles.providerTabActive : ""}`}
                  onClick={() => { setActiveIndex(i); setShowPaths(false); }}
                >
                  <span className={`${styles.providerTabDot} ${styles[dotClass]}`} />
                  {CDN_INFO[r.provider].name}
                </button>
              );
            })}
          </div>
        )}

        {/* 상태 배너 */}
        <div className={`${styles.statusBanner} ${isAllSuccess ? styles.success : isAllFailed ? styles.failed : styles.partial}`}>
          <span className={styles.statusIcon}>
            {isAllSuccess ? "✓" : isAllFailed ? "✗" : "⚠"}
          </span>
          <div className={styles.statusText}>
            <strong>
              {isAllSuccess ? "Purge 완료"
               : isAllFailed ? "Purge 실패"
               : "Purge 부분 완료"}
            </strong>
            <span>
              {isAllSuccess
                ? `${result.totalPaths}개 경로 모두 무효화되었습니다.`
                : isAllFailed
                  ? `${result.totalPaths}개 경로 모두 실패했습니다.`
                  : `성공 ${result.successCount}개 / 실패 ${result.failedCount}개`}
              {isMulti && ` · 전체 CDN 합계: 성공 ${overallSuccess}개 / 실패 ${overallFailed}개`}
            </span>
          </div>
        </div>

        <div className={styles.body}>
          {/* 기본 정보 */}
          <section className={styles.infoSection}>
            <div className={styles.infoGrid}>
              <div className={styles.infoItem}>
                <span className={styles.infoLabel}>CDN 제공자</span>
                <span className={styles.infoValue}>{cdnInfo.name}</span>
              </div>
              {result.domain && (
                <div className={styles.infoItem}>
                  <span className={styles.infoLabel}>CDN 도메인</span>
                  <span className={styles.infoValue}>{result.domain}</span>
                </div>
              )}
              {result.batches[0]?.requestEndpoint && (
                <div className={styles.infoItem} style={{ gridColumn: "1 / -1" }}>
                  <span className={styles.infoLabel}>요청 엔드포인트</span>
                  <span className={styles.infoValue} style={{ fontFamily: "var(--font-family-mono)", fontSize: 11 }}>
                    {result.batches[0].requestEndpoint}
                  </span>
                </div>
              )}
              <div className={styles.infoItem}>
                <span className={styles.infoLabel}>요청 경로 수</span>
                <span className={styles.infoValue}>{result.totalPaths}개
                  {result.batches.length > 1 && (
                    <span className={styles.badge}>{result.batches.length}배치</span>
                  )}
                </span>
              </div>
              <div className={styles.infoItem}>
                <span className={styles.infoLabel}>소요 시간</span>
                <span className={styles.infoValue}>{elapsed}초</span>
              </div>
              <div className={styles.infoItem}>
                <span className={styles.infoLabel}>요청 시각</span>
                <span className={styles.infoValue}>{fmtTime(result.startedAt)}</span>
              </div>
            </div>

            {allInvalidationIds.length > 0 && (
              <div className={styles.invalidationBox}>
                <span className={styles.infoLabel}>{cdnInfo.invalidationLabel ?? "요청 ID"}</span>
                <div className={styles.invalidationIds}>
                  {allInvalidationIds.map((id) => (
                    <code key={id} className={styles.idCode}>{id}</code>
                  ))}
                </div>
              </div>
            )}
          </section>

          {/* 실패 상세 */}
          {failedBatches.length > 0 && (
            <section className={styles.errorSection}>
              <div className={styles.sectionTitle}>오류 내용</div>
              {failedBatches.map((b, i) => (
                <div key={i} className={styles.errorItem}>
                  <span className={styles.errorIcon}>✗</span>
                  <div>
                    <div className={styles.errorMsg}>{b.error ?? "알 수 없는 오류"}</div>
                    <div className={styles.errorPaths}>{b.paths.length}개 경로 영향</div>
                  </div>
                </div>
              ))}
            </section>
          )}

          {/* 경로 목록 (접기/펼치기) */}
          <section className={styles.pathsSection}>
            <button
              className={styles.toggleBtn}
              onClick={() => setShowPaths((v) => !v)}
            >
              {showPaths ? "▾" : "▸"} 무효화 경로 목록 ({allPaths.length}개)
            </button>
            {showPaths && (
              <div className={styles.pathList}>
                {result.batches.map((batch, bi) =>
                  batch.paths.map((path, pi) => (
                    <div
                      key={`${bi}-${pi}`}
                      className={`${styles.pathItem} ${batch.success ? styles.pathOk : styles.pathFail}`}
                    >
                      <span className={styles.pathStatus}>{batch.success ? "✓" : "✗"}</span>
                      <code className={styles.pathCode}>{path}</code>
                      {!batch.success && batch.invalidationId && (
                        <span className={styles.pathBadge}>{batch.invalidationId}</span>
                      )}
                    </div>
                  ))
                )}
              </div>
            )}
          </section>

        </div>

        <div className={styles.footer}>
          <button className={styles.closeFullBtn} onClick={onClose}>닫기</button>
        </div>
      </div>
    </div>,
    document.body
  );
}
