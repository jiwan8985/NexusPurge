import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import ConfirmDialog from "../common/ConfirmDialog";
import type { S3Profile, CdnProvider } from "../../types";
import styles from "./ProfileModal.module.css";

// H-6: lgu, hyosung 제거 — CloudFront와 Akamai만 지원
const CDN_PROVIDERS: { value: CdnProvider; label: string }[] = [
  { value: "cloudfront", label: "AWS CloudFront" },
  { value: "akamai",     label: "Akamai" },
];

const AWS_REGIONS = [
  "ap-northeast-2",
  "ap-northeast-1",
  "ap-southeast-1",
  "us-east-1",
  "us-west-2",
  "eu-west-1",
];

interface FormState {
  name: string;
  region: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
  endpoint: string;
  cdnProvider: CdnProvider | "";
  cdnDistributionId: string;
  cdnDomain: string;
  // H-6: Akamai 전용 필드
  akamaiClientToken: string;
  akamaiClientSecret: string;
  akamaiAccessToken: string;
  akamaiHost: string;
}

const emptyForm = (): FormState => ({
  name: "",
  region: "ap-northeast-2",
  bucket: "",
  accessKeyId: "",
  secretAccessKey: "",
  endpoint: "",
  cdnProvider: "cloudfront",
  cdnDistributionId: "",
  cdnDomain: "",
  akamaiClientToken: "",
  akamaiClientSecret: "",
  akamaiAccessToken: "",
  akamaiHost: "",
});

export default function ProfileModal() {
  const { closeProfileModal } = useAppStore((s) => ({
    closeProfileModal: s.closeProfileModal,
  }));
  const { profiles, saveProfile, deleteProfile, connectWithProfile, testConnection } = useProfile();

  const [form, setForm] = useState<FormState>(emptyForm());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; error?: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);

  const handleEdit = (profile: S3Profile) => {
    setEditingId(profile.id);
    setTestResult(null);
    setError(null);
    setForm({
      name: profile.name,
      region: profile.region,
      bucket: profile.bucket,
      accessKeyId: profile.accessKeyId,
      secretAccessKey: "",  // 보안상 마스킹
      endpoint: profile.endpoint ?? "",
      cdnProvider: profile.cdnProvider ?? "cloudfront",
      cdnDistributionId: profile.cdnDistributionId ?? "",
      cdnDomain: profile.cdnDomain ?? "",
      akamaiClientToken: profile.akamaiClientToken ?? "",
      akamaiClientSecret: "",  // 보안상 마스킹
      akamaiAccessToken: profile.akamaiAccessToken ?? "",
      akamaiHost: profile.akamaiHost ?? "",
    });
  };

  const handleNew = () => {
    setEditingId(null);
    setForm(emptyForm());
    setError(null);
    setTestResult(null);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
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
        accessKeyId: form.accessKeyId,
        secretAccessKey: form.secretAccessKey,
        endpoint: form.endpoint || undefined,
        cdnProvider: (form.cdnProvider as CdnProvider) || undefined,
        cdnDistributionId: form.cdnDistributionId || undefined,
        cdnDomain: form.cdnDomain || undefined,
        akamaiClientToken: form.akamaiClientToken || undefined,
        akamaiClientSecret: form.akamaiClientSecret || undefined,
        akamaiAccessToken: form.akamaiAccessToken || undefined,
        akamaiHost: form.akamaiHost || undefined,
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
    if (!form.bucket || !form.accessKeyId) {
      setError("버킷과 Access Key는 필수입니다.");
      return;
    }
    // 비밀키: 폼에 입력된 값 우선, 없으면 기존 프로파일 사용
    if (!form.secretAccessKey && !editingId) {
      setError("연결 테스트를 위해 Secret Access Key를 입력하세요.");
      return;
    }

    setIsTesting(true);
    setTestResult(null);
    setError(null);

    try {
      if (form.secretAccessKey) {
        // 직접 입력값으로 테스트
        const result = await testConnection({
          region:    form.region,
          bucket:    form.bucket,
          accessKey: form.accessKeyId,
          secretKey: form.secretAccessKey,
          endpoint:  form.endpoint || undefined,
        });
        setTestResult(result);
      } else if (editingId) {
        // 기존 저장된 자격증명으로 테스트 (connect_s3 재사용)
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("connect_s3", { profileId: editingId });
          setTestResult({ success: true });
        } catch (err) {
          setTestResult({ success: false, error: String(err) });
        }
      }
    } finally {
      setIsTesting(false);
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
    setForm((f) => ({ ...f, [field]: e.target.value }));
  };

  const isAkamai = form.cdnProvider === "akamai";
  const isCloudFront = form.cdnProvider === "cloudfront";

  return (
    <>
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
    <div className={styles.overlay} onClick={(e) => e.target === e.currentTarget && closeProfileModal()}>
      <div className={styles.modal}>
        <div className={styles.header}>
          <span className={styles.title}>접속 프로필 관리</span>
          <button className={styles.closeBtn} onClick={closeProfileModal}>✕</button>
        </div>

        <div className={styles.body}>
          {/* 프로필 목록 */}
          <div className={styles.profileList}>
            <div className={styles.sectionHeader}>
              저장된 프로필
              <button className={styles.newBtn} onClick={handleNew}>+ 새 프로필</button>
            </div>

            {profiles.length === 0 ? (
              <div className={styles.empty}>저장된 프로필이 없습니다</div>
            ) : (
              profiles.map((p) => (
                <div
                  key={p.id}
                  className={`${styles.profileItem} ${editingId === p.id ? styles.active : ""}`}
                >
                  <div className={styles.profileInfo} onClick={() => handleEdit(p)}>
                    <span className={styles.profileName}>{p.name}</span>
                    <span className={styles.profileDetail}>
                      {p.bucket} · {p.region}
                    </span>
                  </div>
                  <div className={styles.profileActions}>
                    <button className={styles.connectBtn} onClick={() => handleConnect(p)}>
                      연결
                    </button>
                    <button className={styles.deleteBtn} onClick={() => setDeleteConfirmId(p.id)}>
                      삭제
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>

          {/* 프로필 편집 폼 */}
          <form className={styles.form} onSubmit={handleSubmit}>
            <div className={styles.sectionHeader}>
              {editingId ? "프로필 편집" : "새 프로필"}
            </div>

            {error && <div className={styles.errorMsg}>{error}</div>}

            {/* S3 설정 */}
            <fieldset className={styles.fieldset}>
              <legend>S3 설정</legend>

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
                <select value={form.region} onChange={setField("region")}>
                  {AWS_REGIONS.map((r) => (
                    <option key={r} value={r}>{r}</option>
                  ))}
                </select>
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
                  <span className={testResult.success ? styles.testOk : styles.testFail}>
                    {testResult.success ? "✓ 연결 성공" : `✗ ${testResult.error}`}
                  </span>
                )}
              </div>
            </fieldset>

            {/* CDN 설정 */}
            <fieldset className={styles.fieldset}>
              <legend>CDN 설정 (선택)</legend>

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
            </fieldset>

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
