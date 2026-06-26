import { useRef, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { runtime } from "../../services/runtime";
import ConfirmDialog from "../common/ConfirmDialog";
import type { S3Profile, CdnProvider, CdnConnectionTestResult } from "../../types";
import styles from "./ProfileModal.module.css";

const CDN_PROVIDERS: { value: CdnProvider; label: string }[] = [
  { value: "cloudfront", label: "AWS CloudFront" },
  { value: "akamai",     label: "Akamai" },
  { value: "lguplus",    label: "LG U+ CDN" },
  { value: "kt",         label: "KT CDN" },
  { value: "hyosung",    label: "Hyosung ITX CDN" },
];

const REGION_SUGGESTIONS = [
  "ap-northeast-2",
  "ap-northeast-1",
  "ap-southeast-1",
  "us-east-1",
  "us-west-2",
  "eu-west-1",
  "ap-singapore",
  "auto",
];

const shouldConfirmExternalRequests = () =>
  window.localStorage.getItem("nexuspurge.confirmExternalRequests") !== "false";

const normalizeAccessKeyId = (value: string) => value.trim();
const normalizeSecretAccessKey = (value: string) => value.trim();

interface FormState {
  name: string;
  region: string;
  bucket: string;
  basePrefix: string;
  accessKeyId: string;
  secretAccessKey: string;
  endpoint: string;
  cdnProvider: CdnProvider | "";
  cdnDistributionId: string;
  cdnDomain: string;
  cdnBasePath: string;
  purgeOnNewUpload: boolean;
  defaultCacheControl: string;
  contentTypeOverride: string;
  multipartEtagFallback: boolean;
  // Akamai EdgeGrid
  akamaiClientToken: string;
  akamaiClientSecret: string;
  akamaiAccessToken: string;
  akamaiHost: string;
  // LG U+ CDN
  lguplusUsername: string;
  lguplusPassword: string;
  lguplusServiceName: string;
  lguplusVolumeName: string;
  lguplusEndpoint: string;
  // KT CDN
  ktUsername: string;
  ktPassword: string;
  ktServiceName: string;
  ktVolumeName: string;
  ktEndpoint: string;
  // Hyosung CDN
  hyosungApiKey: string;
  hyosungApiSecret: string;
  hyosungEndpoint: string;
}

const emptyForm = (): FormState => ({
  name: "",
  region: "ap-northeast-2",
  bucket: "",
  basePrefix: "",
  accessKeyId: "",
  secretAccessKey: "",
  endpoint: "",
  cdnProvider: "",
  cdnDistributionId: "",
  cdnDomain: "",
  cdnBasePath: "",
  purgeOnNewUpload: false,
  defaultCacheControl: "",
  contentTypeOverride: "",
  multipartEtagFallback: true,
  akamaiClientToken: "",
  akamaiClientSecret: "",
  akamaiAccessToken: "",
  akamaiHost: "",
  lguplusUsername: "",
  lguplusPassword: "",
  lguplusServiceName: "",
  lguplusVolumeName: "",
  lguplusEndpoint: "",
  ktUsername: "",
  ktPassword: "",
  ktServiceName: "",
  ktVolumeName: "",
  ktEndpoint: "",
  hyosungApiKey: "",
  hyosungApiSecret: "",
  hyosungEndpoint: "",
});

export default function ProfileModal() {
  const { closeProfileModal } = useAppStore((s) => ({
    closeProfileModal: s.closeProfileModal,
  }));
  const { profiles, saveProfile, deleteProfile, connectWithProfile, testConnection, exportProfile, importProfile } = useProfile();

  const [form, setForm] = useState<FormState>(emptyForm());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isTestingCdn, setIsTestingCdn] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; error?: string; warnings?: string[] } | null>(null);
  const [cdnTestResult, setCdnTestResult] = useState<CdnConnectionTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [confirmRequest, setConfirmRequest] = useState<{ message: string; onConfirm: () => void } | null>(null);
  const isLocalStack = form.endpoint.includes("localhost:4566") || form.endpoint.includes("127.0.0.1:4566");

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

  const handleEdit = (profile: S3Profile) => {
    setEditingId(profile.id);
    setTestResult(null);
    setCdnTestResult(null);
    setError(null);
    setForm({
      name: profile.name,
      region: profile.region,
      bucket: profile.bucket,
      basePrefix: profile.basePrefix ?? "",
      accessKeyId: profile.accessKeyId,
      secretAccessKey: "",  // 보안상 마스킹
      endpoint: profile.endpoint ?? "",
      cdnProvider: profile.cdnProvider ?? "",
      cdnDistributionId: profile.cdnDistributionId ?? "",
      cdnDomain: profile.cdnDomain ?? "",
      cdnBasePath: profile.cdnBasePath ?? "",
      purgeOnNewUpload: profile.purgeOnNewUpload ?? false,
      defaultCacheControl: profile.defaultCacheControl ?? "",
      contentTypeOverride: profile.contentTypeOverride ?? "",
      multipartEtagFallback: profile.multipartEtagFallback ?? true,
      akamaiClientToken: profile.akamaiClientToken ?? "",
      akamaiClientSecret: "",  // 보안상 마스킹
      akamaiAccessToken: profile.akamaiAccessToken ?? "",
      akamaiHost: profile.akamaiHost ?? "",
      lguplusUsername: profile.lguplusUsername ?? "",
      lguplusPassword: "",     // 보안상 마스킹
      lguplusServiceName: profile.lguplusServiceName ?? "",
      lguplusVolumeName: profile.lguplusVolumeName ?? "",
      lguplusEndpoint: profile.lguplusEndpoint ?? "",
      ktUsername: profile.ktUsername ?? "",
      ktPassword: "",          // 보안상 마스킹
      ktServiceName: profile.ktServiceName ?? "",
      ktVolumeName: profile.ktVolumeName ?? "",
      ktEndpoint: profile.ktEndpoint ?? "",
      hyosungApiKey: profile.hyosungApiKey ?? "",
      hyosungApiSecret: "",    // 보안상 마스킹
      hyosungEndpoint: profile.hyosungEndpoint ?? "",
    });
  };

  const handleNew = () => {
    setEditingId(null);
    setForm(emptyForm());
    setError(null);
    setTestResult(null);
    setCdnTestResult(null);
  };

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const text = await file.text();
    if (!importPassphrase.trim()) {
      setImportError("패스프레이즈를 입력하세요.");
      return;
    }
    setIsImporting(true);
    setImportError(null);
    try {
      await importProfile(text, importPassphrase.trim());
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

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const accessKeyId = normalizeAccessKeyId(form.accessKeyId);
    const secretAccessKey = normalizeSecretAccessKey(form.secretAccessKey);
    if (!form.name || !form.bucket || !form.accessKeyId) {
      setError("이름, 버킷, Access Key는 필수입니다.");
      return;
    }
    setIsSubmitting(true);
    setError(null);
    try {
      await saveProfile({
        id: editingId ?? crypto.randomUUID(),
        name: form.name,
        region: form.region,
        bucket: form.bucket,
        basePrefix: form.basePrefix || undefined,
        accessKeyId,
        secretAccessKey,
        endpoint: form.endpoint.trim() || undefined,
        cdnProvider: (form.cdnProvider as CdnProvider) || undefined,
        cdnDistributionId: form.cdnDistributionId || undefined,
        cdnDomain: form.cdnDomain || undefined,
        cdnBasePath: form.cdnBasePath || undefined,
        purgeOnNewUpload: form.purgeOnNewUpload,
        defaultCacheControl: form.defaultCacheControl || undefined,
        contentTypeOverride: form.contentTypeOverride || undefined,
        multipartEtagFallback: form.multipartEtagFallback,
        akamaiClientToken: form.akamaiClientToken || undefined,
        akamaiClientSecret: form.akamaiClientSecret || undefined,
        akamaiAccessToken: form.akamaiAccessToken || undefined,
        akamaiHost: form.akamaiHost || undefined,
        lguplusUsername: form.lguplusUsername || undefined,
        lguplusPassword: form.lguplusPassword || undefined,
        lguplusServiceName: form.lguplusServiceName || undefined,
        lguplusVolumeName: form.lguplusVolumeName || undefined,
        lguplusEndpoint: form.lguplusEndpoint || undefined,
        ktUsername: form.ktUsername || undefined,
        ktPassword: form.ktPassword || undefined,
        ktServiceName: form.ktServiceName || undefined,
        ktVolumeName: form.ktVolumeName || undefined,
        ktEndpoint: form.ktEndpoint || undefined,
        hyosungApiKey: form.hyosungApiKey || undefined,
        hyosungApiSecret: form.hyosungApiSecret || undefined,
        hyosungEndpoint: form.hyosungEndpoint || undefined,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      });
      handleNew();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  /** H-3: 저장 없이 입력값으로 연결 테스트 */
  const handleTestConnection = async () => {
    const accessKeyId = normalizeAccessKeyId(form.accessKeyId);
    const secretAccessKey = normalizeSecretAccessKey(form.secretAccessKey);
    if (!form.bucket || !form.accessKeyId) {
      setError("버킷과 Access Key는 필수입니다.");
      return;
    }
    // 비밀키: 폼에 입력된 값 우선, 없으면 기존 프로파일 사용
    if (!form.secretAccessKey && !editingId) {
      setError("연결 테스트를 위해 Secret Access Key를 입력하세요.");
      return;
    }

    if (!isLocalStack && shouldConfirmExternalRequests()) {
      setConfirmRequest({
        message: "실제 AWS/S3-compatible 계정으로 연결 테스트를 실행합니다. 계정 정책에 따라 요청 비용이 발생할 수 있습니다.",
        onConfirm: () => void runS3Test(accessKeyId, secretAccessKey),
      });
      return;
    }
    void runS3Test(accessKeyId, secretAccessKey);
  };

  const runS3Test = async (accessKeyId: string, secretAccessKey: string) => {
    setIsTesting(true);
    setTestResult(null);
    setError(null);

    try {
      if (secretAccessKey) {
        // 직접 입력값으로 테스트
        const result = await testConnection({
          region:    form.region,
          bucket:    form.bucket,
          basePrefix: form.basePrefix,
          accessKey: accessKeyId,
          secretKey: secretAccessKey,
          endpoint:  form.endpoint.trim() || undefined,
        });
        setTestResult(result);
      } else if (editingId) {
        // 기존 저장된 자격증명으로 테스트 (connect_s3 재사용)
        try {
          const result = await runtime.invoke<{ success: boolean; warnings: string[] }>("connect_s3", { profileId: editingId });
          setTestResult(result);
        } catch (err) {
          setTestResult({ success: false, error: String(err) });
        }
      }
    } finally {
      setIsTesting(false);
    }
  };

  const handleTestCdnConnection = async () => {
    if (!editingId) {
      setError("CDN 연결 테스트는 프로파일 저장 후 실행할 수 있습니다.");
      return;
    }
    if (!form.cdnProvider) {
      setError("CDN 제공자를 선택하세요.");
      return;
    }
    if (form.cdnProvider === "cloudfront" && !form.cdnDistributionId) {
      setError("CloudFront Distribution ID를 입력하세요.");
      return;
    }
    if (form.cdnProvider === "akamai" && !form.cdnDomain) {
      setError("CDN 도메인을 입력하세요.");
      return;
    }
    if (form.cdnProvider === "lguplus" && (!form.lguplusUsername || !form.lguplusServiceName)) {
      setError("LG U+ CDN Username과 Service Name을 입력하세요.");
      return;
    }
    if (form.cdnProvider === "kt" && (!form.ktUsername || !form.ktServiceName)) {
      setError("KT CDN Username과 Service Name을 입력하세요.");
      return;
    }
    if (form.cdnProvider === "hyosung" && (!form.hyosungApiKey || !form.cdnDistributionId)) {
      setError("효성 ITX CDN API Key와 Service ID(Distribution ID)를 입력하세요.");
      return;
    }

    if (shouldConfirmExternalRequests()) {
      setConfirmRequest({
        message: "실제 CDN Provider API로 연결 테스트를 실행합니다. CloudFront/Akamai 계정 정책에 따라 요청 비용이 발생할 수 있습니다.",
        onConfirm: () => void runCdnTest(),
      });
      return;
    }
    void runCdnTest();
  };

  const runCdnTest = async () => {
    if (!editingId || !form.cdnProvider) return;
    setIsTestingCdn(true);
    setCdnTestResult(null);
    setError(null);

    try {
      const result = await runtime.invoke<CdnConnectionTestResult>("test_cdn_connection", {
        profileId: editingId,
        provider: form.cdnProvider,
        distributionId: form.cdnDistributionId,
      });
      setCdnTestResult(result);
    } catch (err) {
      setCdnTestResult({
        success: false,
        provider: form.cdnProvider as CdnProvider,
        error: String(err),
      });
    } finally {
      setIsTestingCdn(false);
    }
  };

  const handleConnect = async (profile: S3Profile) => {
    await connectWithProfile(profile);
    closeProfileModal();
  };

  const setField = (field: keyof FormState) => (
    e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>
  ) => {
    setTestResult(null);
    setCdnTestResult(null);
    setForm((f) => ({ ...f, [field]: e.target.value }));
  };

  const setCheckedField = (field: "purgeOnNewUpload" | "multipartEtagFallback") => (
    e: React.ChangeEvent<HTMLInputElement>
  ) => {
    setTestResult(null);
    setCdnTestResult(null);
    setForm((f) => ({ ...f, [field]: e.target.checked }));
  };

  const isAkamai = form.cdnProvider === "akamai";
  const isCloudFront = form.cdnProvider === "cloudfront";
  const isLguplus = form.cdnProvider === "lguplus";
  const isKt = form.cdnProvider === "kt";
  const isHyosung = form.cdnProvider === "hyosung";

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
          if (editingId === deleteConfirmId) handleNew();
          setDeleteConfirmId(null);
        }}
        onCancel={() => setDeleteConfirmId(null)}
      />
    )}

    {/* Import 패스프레이즈 모달 */}
    {showImportModal && (
      <div className={styles.overlay} style={{ zIndex: 200 }}>
        <div className={styles.modal} style={{ maxWidth: 420 }}>
          <div className={styles.header}>
            <span className={styles.title}>프로필 파일 가져오기</span>
            <button type="button" className={styles.closeBtn} onClick={() => { setShowImportModal(false); setImportPassphrase(""); setImportError(null); }}>✕</button>
          </div>
          <div className={styles.body} style={{ display: "block", padding: "1.2rem" }}>
            <p style={{ marginBottom: "0.8rem", fontSize: "0.85rem", opacity: 0.8 }}>
              관리자로부터 받은 <strong>.nexprofile</strong> 파일을 가져옵니다.<br />
              파일 암호화에 사용된 패스프레이즈를 입력하세요.
            </p>
            {importError && <div className={styles.errorMsg}>{importError}</div>}
            <label className={styles.field}>
              <span>패스프레이즈</span>
              <input
                type="password"
                value={importPassphrase}
                onChange={(e) => setImportPassphrase(e.target.value)}
                placeholder="패스프레이즈 입력"
                autoFocus
              />
            </label>
            <div style={{ display: "flex", gap: "0.5rem", marginTop: "1rem" }}>
              <button
                type="button"
                className={styles.saveBtn}
                disabled={isImporting || !importPassphrase.trim()}
                onClick={() => importFileRef.current?.click()}
              >
                {isImporting ? "가져오는 중..." : "파일 선택 및 가져오기"}
              </button>
              <input
                ref={importFileRef}
                type="file"
                accept=".nexprofile,application/octet-stream"
                style={{ display: "none" }}
                onChange={handleImportFile}
              />
            </div>
          </div>
        </div>
      </div>
    )}

    {/* Export 패스프레이즈 모달 */}
    {exportingId && (
      <div className={styles.overlay} style={{ zIndex: 200 }}>
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
      <div className={styles.modal}>
        <div className={styles.header}>
          <span className={styles.title}>접속 프로필 관리</span>
          <button type="button" className={styles.closeBtn} onClick={closeProfileModal}>✕</button>
        </div>

        <div className={styles.body}>
          {/* 프로필 목록 */}
          <div className={styles.profileList}>
            <div className={styles.sectionHeader}>
              저장된 프로필
              <div style={{ display: "flex", gap: "0.4rem" }}>
                <button type="button" className={styles.newBtn} onClick={() => setShowImportModal(true)}>↓ 가져오기</button>
                <button type="button" className={styles.newBtn} onClick={handleNew}>+ 새 프로필</button>
              </div>
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
                <div className={styles.empty}>{search ? "검색 결과가 없습니다" : "저장된 프로필이 없습니다"}</div>
              ) : (
                filteredProfiles.map((p) => (
                  <div
                    key={p.id}
                    className={`${styles.profileItem} ${editingId === p.id ? styles.active : ""}`}
                  >
                    <button type="button" className={styles.profileInfo} onClick={() => handleEdit(p)}>
                      <span className={styles.profileName}>{p.name}</span>
                      <span className={styles.profileDetail}>
                        {p.bucket} · {p.region}
                        {p.permissions?.role && <span style={{ marginLeft: "0.4rem", opacity: 0.6 }}>· {p.permissions.role}</span>}
                      </span>
                    </button>
                    <div className={styles.profileActions}>
                      <button type="button" className={styles.connectBtn} onClick={() => handleConnect(p)}>
                        연결
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
                  </div>
                ))
              )}
            </div>
          </div>

          {/* 프로필 편집 폼 */}
          <form className={styles.form} onSubmit={handleSubmit}>
            <div className={styles.formHeader}>
              <span>{editingId ? "프로필 편집" : "새 프로필"}</span>
            </div>

            <div className={styles.formScroll}>
              {error && <div className={styles.errorMsg}>{error}</div>}
              <div className={isLocalStack ? styles.infoMsg : styles.warnMsg}>
                {isLocalStack
                  ? "LocalStack 프로파일은 로컬 테스트로 비용이 발생하지 않습니다."
                  : "실제 AWS/Akamai 프로파일의 연결 테스트와 CDN 테스트는 계정 사용량에 기록될 수 있습니다."}
              </div>

            {/* S3 설정 */}
            <details className={styles.sectionDetails} open>
              <summary>S3 설정</summary>
              <fieldset className={styles.fieldset}>

              <label className={styles.field}>
                <span>프로파일 이름 *</span>
                <input value={form.name} onChange={setField("name")} placeholder="My S3 Profile" />
              </label>
              <label className={styles.field}>
                <span>버킷 이름 *</span>
                <input value={form.bucket} onChange={setField("bucket")} placeholder="my-bucket" />
              </label>
              <label className={styles.field}>
                <span>리전</span>
                <input
                  value={form.region}
                  onChange={setField("region")}
                  list="region-suggestions"
                  placeholder="us-east-1 / ap-northeast-2 / auto"
                />
                <datalist id="region-suggestions">
                  {REGION_SUGGESTIONS.map((r) => (
                    <option key={r} value={r} />
                  ))}
                </datalist>
              </label>
              <label className={styles.field}>
                <span>Base Prefix</span>
                <input
                  value={form.basePrefix}
                  onChange={setField("basePrefix")}
                  placeholder="prod/ / assets/ / optional"
                />
              </label>
              <label className={styles.field}>
                <span>Access Key ID *</span>
                <input value={form.accessKeyId} onChange={setField("accessKeyId")} placeholder="AKIA..." />
              </label>
              <label className={styles.field}>
                <span>Secret Access Key</span>
                <input
                  type="password"
                  value={form.secretAccessKey}
                  onChange={setField("secretAccessKey")}
                  placeholder={editingId ? "변경하려면 입력" : ""}
                />
              </label>
              <label className={styles.field}>
                <span>커스텀 엔드포인트</span>
                <input value={form.endpoint} onChange={setField("endpoint")} placeholder="https://s3.example.com" />
              </label>
              <label className={styles.field}>
                <span>Cache-Control</span>
                <input
                  value={form.defaultCacheControl}
                  onChange={setField("defaultCacheControl")}
                  list="cache-control-suggestions"
                  placeholder="자동 / no-cache / max-age=31536000, immutable"
                />
                <datalist id="cache-control-suggestions">
                  <option value="no-cache" />
                  <option value="max-age=3600" />
                  <option value="max-age=86400" />
                  <option value="max-age=31536000, immutable" />
                </datalist>
              </label>
              <label className={styles.field}>
                <span>Content-Type override</span>
                <input
                  value={form.contentTypeOverride}
                  onChange={setField("contentTypeOverride")}
                  placeholder="자동 감지 / text/html / application/json"
                />
              </label>
              <label className={styles.field}>
                <span>Multipart ETag fallback</span>
                <input
                  type="checkbox"
                  checked={form.multipartEtagFallback}
                  onChange={setCheckedField("multipartEtagFallback")}
                />
              </label>

              {/* H-3: 연결 테스트 */}
              <div className={styles.testRow}>
                <button
                  type="button"
                  className={styles.testBtn}
                  onClick={handleTestConnection}
                  disabled={isTesting}
                >
                  {isTesting ? "테스트 중..." : "연결 테스트"}
                </button>
                {testResult && (
                  <>
                  <span className={testResult.success ? styles.testOk : styles.testFail}>
                    {testResult.success ? "✓ 연결 성공" : `✗ ${testResult.error}`}
                  </span>
                  {testResult.success && testResult.warnings?.length ? (
                    <span className={styles.warnMsg}>{testResult.warnings.join(" / ")}</span>
                  ) : null}
                  </>
                )}
              </div>
              </fieldset>
            </details>

            {/* CDN 설정 */}
            <details className={styles.sectionDetails} open>
              <summary>CDN 설정</summary>
              <fieldset className={styles.fieldset}>

              <label className={styles.field}>
                <span>CDN 제공자</span>
                <select value={form.cdnProvider} onChange={setField("cdnProvider")}>
                  <option value="">사용 안 함</option>
                  {CDN_PROVIDERS.map((c) => (
                    <option key={c.value} value={c.value}>{c.label}</option>
                  ))}
                </select>
              </label>

              {isCloudFront && (
                <>
                  <label className={styles.field}>
                    <span>Distribution ID</span>
                    <input
                      value={form.cdnDistributionId}
                      onChange={setField("cdnDistributionId")}
                      placeholder="EDFDVBD6EXAMPLE"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>CDN 도메인</span>
                    <input
                      value={form.cdnDomain}
                      onChange={setField("cdnDomain")}
                      placeholder="d111111abcdef8.cloudfront.net"
                    />
                  </label>
                </>
              )}

              {isAkamai && (
                <>
                  <label className={styles.field}>
                    <span>EdgeGrid 호스트</span>
                    <input
                      value={form.akamaiHost}
                      onChange={setField("akamaiHost")}
                      placeholder="akab-xxxx.luna.akamaiapis.net"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Client Token</span>
                    <input
                      value={form.akamaiClientToken}
                      onChange={setField("akamaiClientToken")}
                      placeholder="akab-xxxx..."
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Access Token</span>
                    <input
                      value={form.akamaiAccessToken}
                      onChange={setField("akamaiAccessToken")}
                      placeholder="akab-yyyy..."
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Client Secret</span>
                    <input
                      type="password"
                      value={form.akamaiClientSecret}
                      onChange={setField("akamaiClientSecret")}
                      placeholder={editingId ? "변경하려면 입력" : ""}
                    />
                  </label>
                  <label className={styles.field}>
                    <span>CDN 도메인 (Purge URL 기준)</span>
                    <input
                      value={form.cdnDomain}
                      onChange={setField("cdnDomain")}
                      placeholder="cdn.example.com"
                    />
                  </label>
                </>
              )}

              {isLguplus && (
                <>
                  <label className={styles.field}>
                    <span>Username *</span>
                    <input
                      value={form.lguplusUsername}
                      onChange={setField("lguplusUsername")}
                      placeholder="LG U+ CDN 계정 아이디"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Password *</span>
                    <input
                      type="password"
                      value={form.lguplusPassword}
                      onChange={setField("lguplusPassword")}
                      placeholder={editingId ? "변경하려면 입력" : "LG U+ CDN 계정 비밀번호"}
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Service Name *</span>
                    <input
                      value={form.lguplusServiceName}
                      onChange={setField("lguplusServiceName")}
                      placeholder="서비스 이름 (SERVICE_NAME)"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Volume Name</span>
                    <input
                      value={form.lguplusVolumeName}
                      onChange={setField("lguplusVolumeName")}
                      placeholder="볼륨 이름 (VOLUME_NAME)"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>CDN 도메인 (FQDN) *</span>
                    <input
                      value={form.cdnDomain}
                      onChange={setField("cdnDomain")}
                      placeholder="cdn.example.com"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>API 엔드포인트</span>
                    <input
                      value={form.lguplusEndpoint}
                      onChange={setField("lguplusEndpoint")}
                      placeholder="https://api.lgucdn.com (기본값)"
                    />
                  </label>
                </>
              )}

              {isKt && (
                <>
                  <label className={styles.field}>
                    <span>Username *</span>
                    <input
                      value={form.ktUsername}
                      onChange={setField("ktUsername")}
                      placeholder="KT CDN 계정 아이디"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Password *</span>
                    <input
                      type="password"
                      value={form.ktPassword}
                      onChange={setField("ktPassword")}
                      placeholder={editingId ? "변경하려면 입력" : "KT CDN 계정 비밀번호"}
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Service Name *</span>
                    <input
                      value={form.ktServiceName}
                      onChange={setField("ktServiceName")}
                      placeholder="서비스 이름 (SERVICE_NAME)"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Volume Name</span>
                    <input
                      value={form.ktVolumeName}
                      onChange={setField("ktVolumeName")}
                      placeholder="볼륨 이름 (VOLUME_NAME)"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>CDN 도메인 (FQDN) *</span>
                    <input
                      value={form.cdnDomain}
                      onChange={setField("cdnDomain")}
                      placeholder="cdn.example.com"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>API 엔드포인트</span>
                    <input
                      value={form.ktEndpoint}
                      onChange={setField("ktEndpoint")}
                      placeholder="https://api.ktcdn.co.kr (기본값)"
                    />
                  </label>
                </>
              )}

              {isHyosung && (
                <>
                  <label className={styles.field}>
                    <span>API Key (Principal) *</span>
                    <input
                      value={form.hyosungApiKey}
                      onChange={setField("hyosungApiKey")}
                      placeholder="효성 ITX CDN API Key"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>API Secret *</span>
                    <input
                      type="password"
                      value={form.hyosungApiSecret}
                      onChange={setField("hyosungApiSecret")}
                      placeholder={editingId ? "변경하려면 입력" : "효성 ITX CDN API Secret"}
                    />
                  </label>
                  <label className={styles.field}>
                    <span>Service ID (Distribution ID) *</span>
                    <input
                      value={form.cdnDistributionId}
                      onChange={setField("cdnDistributionId")}
                      placeholder="TID_XXXXX"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>CDN 도메인 *</span>
                    <input
                      value={form.cdnDomain}
                      onChange={setField("cdnDomain")}
                      placeholder="cdn.example.com"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>API 엔드포인트</span>
                    <input
                      value={form.hyosungEndpoint}
                      onChange={setField("hyosungEndpoint")}
                      placeholder="https://api.xtrmcdn.co.kr:28091 (기본값)"
                    />
                  </label>
                </>
              )}

              {form.cdnProvider && (
                <div className={styles.testRow}>
                  <button
                    type="button"
                    className={styles.testBtn}
                    onClick={handleTestCdnConnection}
                    disabled={isTestingCdn}
                  >
                    {isTestingCdn ? "CDN 테스트 중..." : "CDN 연결 테스트"}
                  </button>
                  {cdnTestResult && (
                    <span className={cdnTestResult.success ? styles.testOk : styles.testFail}>
                      {cdnTestResult.success
                        ? `✓ CDN 연결 성공${cdnTestResult.domain ? ` · ${cdnTestResult.domain}` : ""}`
                        : `✗ ${cdnTestResult.error}`}
                    </span>
                  )}
                </div>
              )}

              {form.cdnProvider && (
                <label className={styles.field}>
                  <span>CDN Base Path (S3 → CDN 경로 변환)</span>
                  <input
                    value={form.cdnBasePath}
                    onChange={setField("cdnBasePath")}
                    placeholder="예: contents/ (S3 키에서 이 접두사를 제거해 CDN URL 구성)"
                  />
                  <small className={styles.helpText}>
                    S3 키가 <code>contents/file.txt</code>이고 CDN이 <code>/file.txt</code>로 서빙한다면 <code>contents/</code> 입력
                  </small>
                </label>
              )}
              </fieldset>
            </details>

            <details className={styles.sectionDetails} open>
              <summary>Purge 정책</summary>
              <fieldset className={styles.fieldset}>
                <label className={styles.field}>
                  <span>신규 업로드도 Purge</span>
                  <input
                    type="checkbox"
                    checked={form.purgeOnNewUpload}
                    onChange={setCheckedField("purgeOnNewUpload")}
                    disabled={!form.cdnProvider}
                  />
                  <small className={styles.helpText}>
                    기본값은 덮어쓰기 파일만 Purge합니다. 이 옵션을 켜면 새 파일도 업로드 직후 CDN 캐시 무효화 대상으로 보냅니다.
                  </small>
                </label>
              </fieldset>
            </details>

            </div>

            <div className={styles.formActions}>
              <button type="button" onClick={handleNew} className={styles.cancelBtn}>
                취소
              </button>
              <button type="submit" className={styles.saveBtn} disabled={isSubmitting}>
                {isSubmitting ? "저장 중..." : "저장"}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
    </>
  );
}
