import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import type { S3Profile, CdnProvider } from "../../types";
import styles from "./ProfileModal.module.css";

const CDN_PROVIDERS: { value: CdnProvider; label: string }[] = [
  { value: "cloudfront", label: "AWS CloudFront" },
  { value: "akamai", label: "Akamai" },
  { value: "lgu", label: "LG U+" },
  { value: "hyosung", label: "효성 ITX CDN" },
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
});

export default function ProfileModal() {
  const { closeProfileModal } = useAppStore((s) => ({
    closeProfileModal: s.closeProfileModal,
  }));
  const { profiles, saveProfile, deleteProfile, connectWithProfile } = useProfile();

  const [form, setForm] = useState<FormState>(emptyForm());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleEdit = (profile: S3Profile) => {
    setEditingId(profile.id);
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
    });
  };

  const handleNew = () => {
    setEditingId(null);
    setForm(emptyForm());
    setError(null);
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

  const handleConnect = async (profile: S3Profile) => {
    await connectWithProfile(profile);
    closeProfileModal();
  };

  const setField = (field: keyof FormState) => (
    e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>
  ) => setForm((f) => ({ ...f, [field]: e.target.value }));

  return (
    <div className={styles.overlay} onClick={(e) => e.target === e.currentTarget && closeProfileModal()}>
      <div className={styles.modal}>
        <div className={styles.header}>
          <span className={styles.title}>접속 프로파일 관리</span>
          <button className={styles.closeBtn} onClick={closeProfileModal}>✕</button>
        </div>

        <div className={styles.body}>
          {/* 프로파일 목록 */}
          <div className={styles.profileList}>
            <div className={styles.sectionHeader}>
              저장된 프로파일
              <button className={styles.newBtn} onClick={handleNew}>+ 새 프로파일</button>
            </div>

            {profiles.length === 0 ? (
              <div className={styles.empty}>프로파일이 없습니다</div>
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
                    <button
                      className={styles.connectBtn}
                      onClick={() => handleConnect(p)}
                    >
                      연결
                    </button>
                    <button
                      className={styles.deleteBtn}
                      onClick={() => deleteProfile(p.id)}
                    >
                      삭제
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>

          {/* 프로파일 편집 폼 */}
          <form className={styles.form} onSubmit={handleSubmit}>
            <div className={styles.sectionHeader}>
              {editingId ? "프로파일 편집" : "새 프로파일"}
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
              {form.cdnProvider && (
                <>
                  <label className={styles.field}>
                    <span>Distribution ID</span>
                    <input value={form.cdnDistributionId} onChange={setField("cdnDistributionId")} placeholder="EDFDVBD6EXAMPLE" />
                  </label>
                  <label className={styles.field}>
                    <span>CDN 도메인</span>
                    <input value={form.cdnDomain} onChange={setField("cdnDomain")} placeholder="d111111abcdef8.cloudfront.net" />
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
  );
}
