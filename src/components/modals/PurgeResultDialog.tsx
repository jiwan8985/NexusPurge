import { useState } from "react";
import { createPortal } from "react-dom";
import type { PurgeExecutionResult, CdnProvider } from "../../types";
import styles from "./PurgeResultDialog.module.css";

// CDN 제공자별 정보
const CDN_INFO: Record<CdnProvider, {
  name: string;
  propagationTime: string;
  propagationNote: string;
  invalidationLabel?: string;
}> = {
  cloudfront: {
    name:              "AWS CloudFront",
    propagationTime:   "약 5 ~ 15분",
    propagationNote:   "전 세계 500개 이상의 엣지 서버에 순차적으로 반영됩니다.",
    invalidationLabel: "Invalidation ID",
  },
  akamai: {
    name:            "Akamai",
    propagationTime: "수 초 ~ 수 분",
    propagationNote: "Fast Purge 사용 시 수 초 내 반영됩니다.",
  },
  lguplus: {
    name:              "LG U+ CDN",
    propagationTime:   "제공사 정책에 따라 다름",
    propagationNote:   "비동기 트랜잭션으로 처리되며, 진행 상황 조회가 가능합니다.",
    invalidationLabel: "Transaction ID",
  },
  hyosung: {
    name:              "Hyosung ITX CDN",
    propagationTime:   "제공사 정책에 따라 다름",
    propagationNote:   "비동기 트랜잭션으로 처리됩니다.",
    invalidationLabel: "Transaction ID",
  },
  kt: {
    name:              "KT CDN",
    propagationTime:   "제공사 정책에 따라 다름",
    propagationNote:   "비동기 트랜잭션으로 처리되며, 진행 상황 조회가 가능합니다.",
    invalidationLabel: "Transaction ID",
  },
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
  result: PurgeExecutionResult;
  onClose: () => void;
}

export default function PurgeResultDialog({ result, onClose }: Props) {
  const [showPaths, setShowPaths] = useState(false);
  const [showExplain, setShowExplain] = useState(false);

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
          <span className={styles.title}>CDN Purge 결과</span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

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
                {result.provider === "cloudfront" && (
                  <small className={styles.helpText}>
                    AWS 콘솔 → CloudFront → Distributions → Invalidations에서 상태를 확인할 수 있습니다.
                  </small>
                )}
              </div>
            )}
          </section>

          {/* 반영 예상 시간 */}
          <section className={`${styles.propagationBox} ${isAllSuccess ? styles.propagationOk : isAllFailed ? styles.propagationErr : styles.propagationWarn}`}>
            <div className={styles.propagationHeader}>
              <span className={styles.clockIcon}>⏱</span>
              <div>
                <strong>캐시 반영 예상 시간: {cdnInfo.propagationTime}</strong>
                <p>{cdnInfo.propagationNote}</p>
              </div>
            </div>
            {isAllSuccess && (
              <p className={styles.propagationTip}>
                이 시간 동안은 일부 사용자에게 이전 버전이 보일 수 있습니다. 이는 정상적인 동작입니다.
              </p>
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
              <div className={styles.errorHelp}>
                <strong>해결 방법</strong>
                <ul>
                  <li>CDN 자격증명(API Key, Secret)이 올바른지 확인하세요.</li>
                  <li>프로필 수정 후 CDN 연결 테스트를 실행하세요.</li>
                  <li>CDN 제공자의 API 상태 페이지를 확인하세요.</li>
                  <li>문제가 지속되면 CDN 관리자에게 문의하세요.</li>
                </ul>
              </div>
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

          {/* CDN 설명 (CDN을 모르는 사용자를 위한 설명) */}
          <section className={styles.explainSection}>
            <button
              className={styles.toggleBtn}
              onClick={() => setShowExplain((v) => !v)}
            >
              {showExplain ? "▾" : "▸"} CDN Purge란 무엇인가요?
            </button>
            {showExplain && (
              <div className={styles.explainContent}>
                <p>
                  <strong>CDN(Content Delivery Network)</strong>은 파일을 전 세계 여러 서버에 복사해
                  사용자에게 가장 가까운 서버에서 빠르게 전달하는 시스템입니다.
                </p>
                <p>
                  파일을 S3에 업로드해도 CDN에는 이전 버전이 남아 있을 수 있습니다.
                  <strong> Purge(퍼지)</strong>는 CDN에 저장된 오래된 복사본을 강제로 삭제해,
                  사용자가 다음 요청 시 S3에서 최신 파일을 받도록 만드는 작업입니다.
                </p>
                <div className={styles.explainFlow}>
                  <div className={styles.flowStep}>
                    <span className={styles.flowNum}>1</span>
                    <span>S3에 새 파일 업로드</span>
                  </div>
                  <span className={styles.flowArrow}>→</span>
                  <div className={styles.flowStep}>
                    <span className={styles.flowNum}>2</span>
                    <span>CDN Purge 요청</span>
                  </div>
                  <span className={styles.flowArrow}>→</span>
                  <div className={styles.flowStep}>
                    <span className={styles.flowNum}>3</span>
                    <span>CDN 캐시 삭제 ({cdnInfo.propagationTime})</span>
                  </div>
                  <span className={styles.flowArrow}>→</span>
                  <div className={styles.flowStep}>
                    <span className={styles.flowNum}>4</span>
                    <span>사용자에게 최신 파일 전달</span>
                  </div>
                </div>
                {result.provider === "cloudfront" && (
                  <p className={styles.explainNote}>
                    <strong>CloudFront</strong>는 무효화(Invalidation) 방식을 사용합니다.
                    요청 후 5~15분 내에 전 세계 엣지 서버에서 캐시가 삭제되며,
                    그 사이에는 일부 사용자가 이전 버전을 볼 수 있습니다.
                  </p>
                )}
                {result.provider === "akamai" && (
                  <p className={styles.explainNote}>
                    <strong>Akamai</strong>는 Fast Purge를 지원하여 수 초 내에 반영됩니다.
                    대량 Purge 시 초당 요청 수 제한이 있을 수 있습니다.
                  </p>
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
