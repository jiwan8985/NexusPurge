import type { AuthAdapter, AuthSession } from "./auth-types";

export class ExternalAuthAdapter implements AuthAdapter {
  async login(): Promise<AuthSession> {
    throw new Error("External auth module is not configured yet.");
  }

  async logout(): Promise<void> {
    throw new Error("External auth module is not configured yet.");
  }

  async refreshToken(_session: AuthSession): Promise<AuthSession> {
    throw new Error("External auth module is not configured yet.");
  }

  async getCurrentSession(): Promise<AuthSession | null> {
    return null;
  }
}
