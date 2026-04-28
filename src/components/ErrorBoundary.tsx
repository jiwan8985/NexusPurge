import { Component, type ErrorInfo, type ReactNode } from "react";
import styles from "./ErrorBoundary.module.css";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (error) {
      return (
        <div className={styles.container}>
          <div className={styles.card}>
            <div className={styles.iconWrap}>
              <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
                <path
                  d="M16 3L29 26H3L16 3Z"
                  stroke="currentColor"
                  strokeWidth="1.8"
                  strokeLinejoin="round"
                />
                <path d="M16 13v6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
                <circle cx="16" cy="22.5" r="1" fill="currentColor" />
              </svg>
            </div>
            <h2 className={styles.title}>렌더링 오류가 발생했습니다</h2>
            <p className={styles.message}>{error.message}</p>
            <details className={styles.detail}>
              <summary>스택 트레이스 보기</summary>
              <pre className={styles.stack}>{error.stack}</pre>
            </details>
            <button
              className={styles.retryBtn}
              onClick={() => this.setState({ error: null })}
            >
              다시 시도
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
