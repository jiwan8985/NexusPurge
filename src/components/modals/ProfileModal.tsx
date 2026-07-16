import { useRef, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { runtime } from "../../services/runtime";
import ConfirmDialog from "../common/ConfirmDialog";
import { availableCdns, cdnDistributionIdFor, CDN_LABELS } from "../../utils/cdn";
import type { S3Profile, CdnConnectionTestResult } from "../../types";
import styles from "./ProfileModal.module.css";

// 설정 UI에서 제거됨 — 명시적으로 "true"를 넣은 경우에만 테스트 전 확인 창 표시
const shouldConfirmExternalRequests = () =>
  window.localStorage.getItem("nexuspurge.confirmExternalRequests") === "true";

/**
 * 접속 프로필 관리 — 프로필은 파일 가져오기(.json / .nexprofile)로만 등록한다.
 * 저장된 프로필은 write-only: 이름만 표시되며 [연결]/[테스트]/[내보내기]/[삭제]만 가능,
 * 어떤 설정값도 다시 열람할 수 없다 (고객사 전달용 정책).
 */
export default function ProfileModal() {
  const { closeProfileModal } = useAppStore((s) => ({
    closeProfileModal: s.closeProfileModal,
  }));
  const { profiles, deleteProfile, connectWithProfile, exportProfile, importProfile, loadProfiles } = useProfile();

  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [confirmRequest, setConfirmRequest] = useState<{ message: string; onConfirm: () => void } | null>(null);

  // 목록 행 [테스트] 결과 (✓/✗만 표시, 설정값 비노출)
  const [rowTests, setRowTests] = useState<
    Record<string, { testing: boolean; lines: { label: string; success: boolean; error?: string }[] }>
  >({});

  // 검색
  const [search, setSearch] = useState("");

  // 암호화 프로필 Import
  const [showImportModal, setShowImportModal] = useState(false);
  const [importPassphrase, setImportPassphrase] = useState("");
  const [importError, setImportError] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const importFileRef = useRef<HTMLInputElement>(null);

  // 암호화 프로필 Export
  const [exportingId, setExportingId] = useState<string | null>(null);
  const [exportPassphrase, setExportPassphrase] = useState("");
  const [exportError, setExportError] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);

  /** 목록 행 [테스트]: 저장된 자격증명으로 S3 + 구성된 CDN 연결을 검사 (설정값 비노출) */
  const handleRowTest = async (p: S3Profile) => {
    setRowTests((s) => ({ ...s, [p.id]: { testing: true, lines: [] } }));
    const lines: { label: string; success: boolean; error?: string }[] = [];
    try {
      await runtime.invoke("connect_s3", { profileId: p.id });
      lines.push({ label: "S3", success: true });
    } catch (err) {
      lines.push({ label: "S3", success: false, error: String(err) });
    }
    for (const provider of availableCdns(p)) {
      try {
        const result = await runtime.invoke<CdnConnectionTestResult>("test_cdn_connection", {
          profileId: p.id,
          provider,
          distributionId: cdnDistributionIdFor(p, provider) ?? "",
        });
        lines.push({ label: CDN_LABELS[provider], success: result.success, error: result.error });
      } catch (err) {
        lines.push({ label: CDN_LABELS[provider], success: false, error: String(err) });
      }
    }
    setRowTests((s) => ({ ...s, [p.id]: { testing: false, lines } }));
  };

  const requestRowTest = (p: S3Profile) => {
    if (shouldConfirmExternalRequests()) {
      setConfirmRequest({
        message: "실제 S3/CDN Provider API로 연결 테스트를 실행합니다. 계정 정책에 따라 요청 비용이 발생할 수 있습니다.",
        onConfirm: () => void handleRowTest(p),
      });
      return;
    }
    void handleRowTest(p);
  };

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    // Windows 편집기가 저장 시 붙이는 UTF-8 BOM 제거 (JSON 인식·파싱 오류 방지)
    const text = (await file.text()).replace(/^﻿/, "");

    // 확장자로 판별: .nexprofile은 암호화 파일. 내용은 둘 다 JSON 객체({...})라서
    // startsWith("{") 같은 내용 기반 판별은 암호화 파일을 오탐(평문으로 인식)한다.
    const isPlainJson = !file.name.toLowerCase().endsWith(".nexprofile");
    if (!isPlainJson && !importPassphrase.trim()) {
      setImportError("암호화된 프로필(.nexprofile)은 패스프레이즈를 입력하세요.");
      return;
    }
    setIsImporting(true);
    setImportError(null);
    try {
      if (isPlainJson) {
        await runtime.invoke("import_profile_file", { content: text });
        await loadProfiles();
      } else {
        await importProfile(text, importPassphrase.trim());
      }
      setShowImportModal(false);
      setImportPassphrase("");
    } catch (err) {
      setImportError(String(err));
    } finally {
      setIsImporting(false);
      if (importFileRef.current) importFileRef.current.value = "";
    }
  };

  const handleExportProfile = async () => {
    if (!exportingId || !exportPassphrase.trim()) {
      setExportError("패스프레이즈를 입력하세요.");
      return;
    }
    setIsExporting(true);
    setExportError(null);
    try {
      const encrypted = await exportProfile(exportingId, exportPassphrase.trim());
      const profile = profiles.find((p) => p.id === exportingId);
      const filename = `${profile?.name ?? "profile"}.nexprofile`;
      const blob = new Blob([encrypted], { type: "application/octet-stream" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setExportingId(null);
      setExportPassphrase("");
    } catch (err) {
      setExportError(String(err));
    } finally {
      setIsExporting(false);
    }
  };

  const handleConnect = async (profile: S3Profile) => {
    await connectWithProfile(profile);
    closeProfileModal();
  };

  const filteredProfiles = profiles.filter(
    (p) =>
      !search.trim() ||
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.bucket.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <>
    {confirmRequest && (
      <ConfirmDialog
        title="외부 API 요청 확인"
        message={<p>{confirmRequest.message}</p>}
        confirmLabel="계속 진행"
        onConfirm={() => {
          const req = confirmRequest;
          setConfirmRequest(null);
          req.onConfirm();
        }}
        onCancel={() => setConfirmRequest(null)}
      />
    )}

    {deleteConfirmId && (
      <ConfirmDialog
        title="프로필 삭제"
        message={
          <>
            <p>
              <strong>
                {profiles.find((p) => p.id === deleteConfirmId)?.name}
              </strong>{" "}
              프로필을 삭제합니다.
            </p>
            <p>저장된 자격증명도 함께 삭제됩니다. 이 작업은 취소할 수 없습니다.</p>
          </>
        }
        confirmLabel="삭제"
        danger
        onConfirm={() => {
          deleteProfile(deleteConfirmId);
          setDeleteConfirmId(null);
        }}
        onCancel={() => setDeleteConfirmId(null)}
      />
    )}

    {/* Import 패스프레이즈 모달 — 본체 overlay(z-index 1000)보다 위에 표시 */}
    {showImportModal && (
      <div className={styles.overlay} style={{ zIndex: 1100 }}>
        <div className={styles.modal} style={{ maxWidth: 420 }}>
          <div className={styles.header}>
            <span className={styles.title}>프로필 파일 가져오기</span>
            <button type="button" className={styles.closeBtn} onClick={() => { setShowImportModal(false); setImportPassphrase(""); setImportError(null); }}>✕</button>
          </div>
          <div className={styles.body} style={{ display: "block", padding: "1.2rem" }}>
            <p style={{ marginBottom: "0.8rem", fontSize: "0.85rem", opacity: 0.8 }}>
              프로필 파일을 가져옵니다.<br />
              · <strong>.json</strong> — 테스트용 (패스프레이즈 불필요, 여러 CDN 포함 가능)<br />
              · <strong>.nexprofile</strong> — 암호화 파일 (패스프레이즈 필요)
            </p>
            {importError && <div className={styles.errorMsg}>{importError}</div>}
            <label className={styles.field}>
              <span>패스프레이즈 (.nexprofile 파일만)</span>
              <input
                type="password"
                value={importPassphrase}
                onChange={(e) => setImportPassphrase(e.target.value)}
                placeholder="JSON 파일은 비워두세요"
                autoFocus
              />
            </label>
            <div style={{ display: "flex", gap: "0.5rem", marginTop: "1rem" }}>
              <button
                type="button"
                className={styles.saveBtn}
                disabled={isImporting}
                onClick={() => importFileRef.current?.click()}
              >
                {isImporting ? "가져오는 중..." : "파일 선택 및 가져오기"}
              </button>
              <input
                ref={importFileRef}
                type="file"
                accept=".json,.nexprofile,application/json,application/octet-stream"
                style={{ display: "none" }}
                onChange={handleImportFile}
              />
            </div>
          </div>
        </div>
      </div>
    )}

    {/* Export 패스프레이즈 모달 — 본체 overlay(z-index 1000)보다 위에 표시 */}
    {exportingId && (
      <div className={styles.overlay} style={{ zIndex: 1100 }}>
        <div className={styles.modal} style={{ maxWidth: 420 }}>
          <div className={styles.header}>
            <span className={styles.title}>프로필 내보내기</span>
            <button type="button" className={styles.closeBtn} onClick={() => { setExportingId(null); setExportPassphrase(""); setExportError(null); }}>✕</button>
          </div>
          <div className={styles.body} style={{ display: "block", padding: "1.2rem" }}>
            <p style={{ marginBottom: "0.8rem", fontSize: "0.85rem", opacity: 0.8 }}>
              <strong>{profiles.find((p) => p.id === exportingId)?.name}</strong> 프로필을<br />
              AES-256-GCM 암호화 파일(.nexprofile)로 내보냅니다.
            </p>
            {exportError && <div className={styles.errorMsg}>{exportError}</div>}
            <label className={styles.field}>
              <span>패스프레이즈</span>
              <input
                type="password"
                value={exportPassphrase}
                onChange={(e) => { setExportPassphrase(e.target.value); setExportError(null); }}
                placeholder="암호화에 사용할 패스프레이즈"
                autoFocus
              />
            </label>
            <div style={{ display: "flex", gap: "0.5rem", marginTop: "1rem" }}>
              <button
                type="button"
                className={styles.saveBtn}
                disabled={isExporting || !exportPassphrase.trim()}
                onClick={handleExportProfile}
              >
                {isExporting ? "내보내는 중..." : "내보내기"}
              </button>
            </div>
          </div>
        </div>
      </div>
    )}

    <div className={styles.overlay} onClick={(e) => e.target === e.currentTarget && closeProfileModal()}>
      <div className={`${styles.modal} ${styles.modalCompact}`}>
        <div className={styles.header}>
          <span className={styles.title}>접속 프로필 관리</span>
          <button type="button" className={styles.closeBtn} onClick={closeProfileModal}>✕</button>
        </div>

        <div className={styles.body}>
          {/* 프로필 목록 — 프로필 등록은 파일 가져오기로만 가능 */}
          <div className={`${styles.profileList} ${styles.profileListFull}`}>
            <div className={styles.sectionHeader}>
              저장된 프로필
              <button type="button" className={styles.newBtn} onClick={() => setShowImportModal(true)}>↓ 가져오기</button>
            </div>

            {/* 프로필 검색 */}
            <div style={{ padding: "0.4rem 0.6rem" }}>
              <input
                className={styles.searchInput}
                placeholder="프로필 이름 / 버킷 검색..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>

            <div className={styles.profileItems}>
              {filteredProfiles.length === 0 ? (
                <div className={styles.empty}>
                  {search ? "검색 결과가 없습니다" : "저장된 프로필이 없습니다. [가져오기]로 전달받은 프로필 파일을 등록하세요."}
                </div>
              ) : (
                filteredProfiles.map((p) => (
                  <div key={p.id} className={styles.profileItem}>
                    {/* 고객사 요청: 저장된 프로필은 write-only — 이름만 표시, 클릭해도 아무 정보도 열리지 않음 */}
                    <div className={styles.profileInfo}>
                      <span className={styles.profileName}>{p.name}</span>
                      {p.permissions?.role && (
                        <span className={styles.profileDetail} style={{ opacity: 0.6 }}>{p.permissions.role}</span>
                      )}
                    </div>
                    <div className={styles.profileActions}>
                      <button type="button" className={styles.connectBtn} onClick={() => handleConnect(p)}>
                        연결
                      </button>
                      <button
                        type="button"
                        className={styles.testBtn}
                        title="저장된 자격증명으로 S3/CDN 연결 검사"
                        disabled={rowTests[p.id]?.testing}
                        onClick={() => requestRowTest(p)}
                      >
                        {rowTests[p.id]?.testing ? "테스트 중" : "테스트"}
                      </button>
                      <button
                        type="button"
                        className={styles.testBtn}
                        title="암호화 파일로 내보내기"
                        onClick={() => { setExportingId(p.id); setExportPassphrase(""); setExportError(null); }}
                      >
                        내보내기
                      </button>
                      <button type="button" className={styles.deleteBtn} onClick={() => setDeleteConfirmId(p.id)}>
                        삭제
                      </button>
                    </div>
                    {rowTests[p.id]?.lines.length ? (
                      <div className={styles.testResultLines}>
                        {rowTests[p.id].lines.map((line) => (
                          <span
                            key={line.label}
                            className={line.success ? styles.testOk : styles.testFail}
                            title={line.error}
                          >
                            {line.success ? "✓" : "✗"} {line.label}
                            {!line.success && line.error ? ` — ${line.error}` : ""}
                          </span>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
    </>
  );
}
