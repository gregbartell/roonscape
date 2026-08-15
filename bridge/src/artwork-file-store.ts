import { randomUUID } from "node:crypto";
import {
  chmod,
  mkdir,
  readdir,
  rename,
  unlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";

export interface ArtworkFileReference {
  revision: number;
  path: string;
}

export interface ArtworkFiles {
  stage(revision: number, image: Uint8Array): Promise<ArtworkFileReference>;
  commit(reference: ArtworkFileReference): Promise<void>;
  discard(reference: ArtworkFileReference): Promise<void>;
  clear(): Promise<void>;
}

export class ArtworkFileStore implements ArtworkFiles {
  readonly #directory: string;
  #currentPath: string | undefined;
  #stagedPath: string | undefined;
  #pending: Promise<void> = Promise.resolve();

  private constructor(directory: string) {
    this.#directory = path.resolve(directory);
  }

  static async open(directory: string): Promise<ArtworkFileStore> {
    const store = new ArtworkFileStore(directory);
    await mkdir(store.#directory, { mode: 0o700, recursive: true });
    await chmod(store.#directory, 0o700);
    await store.clear();
    return store;
  }

  stage(revision: number, image: Uint8Array): Promise<ArtworkFileReference> {
    if (!Number.isSafeInteger(revision) || revision < 0) {
      return Promise.reject(new Error("Artwork revision must be non-negative"));
    }

    return this.#enqueue(async () => {
      const fileName = `artwork-${revision}-${randomUUID()}.jpg`;
      const finalPath = path.join(this.#directory, fileName);
      const temporaryPath = path.join(
        this.#directory,
        `.${fileName}.${randomUUID()}.tmp`,
      );

      try {
        await writeFile(temporaryPath, image, { flag: "wx", mode: 0o600 });
        await rename(temporaryPath, finalPath);
      } catch (error) {
        await removeIfPresent(temporaryPath);
        throw error;
      }

      if (
        this.#stagedPath !== undefined &&
        this.#stagedPath !== this.#currentPath &&
        this.#stagedPath !== finalPath
      ) {
        await removeIfPresent(this.#stagedPath);
      }
      this.#stagedPath = finalPath;

      return { revision, path: finalPath };
    });
  }

  commit(reference: ArtworkFileReference): Promise<void> {
    return this.#enqueue(async () => {
      const previousPath = this.#currentPath;
      this.#currentPath = reference.path;
      if (this.#stagedPath === reference.path) {
        this.#stagedPath = undefined;
      }

      if (previousPath !== undefined && previousPath !== reference.path) {
        await removeIfPresent(previousPath);
      }
      await this.#removeObsoleteFiles(reference.path);
    });
  }

  discard(reference: ArtworkFileReference): Promise<void> {
    return this.#enqueue(async () => {
      if (reference.path === this.#currentPath) {
        return;
      }
      if (reference.path === this.#stagedPath) {
        this.#stagedPath = undefined;
      }
      await removeIfPresent(reference.path);
    });
  }

  clear(): Promise<void> {
    return this.#enqueue(async () => {
      this.#currentPath = undefined;
      this.#stagedPath = undefined;
      await this.#removeObsoleteFiles();
    });
  }

  #enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.#pending.then(operation, operation);
    this.#pending = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }

  async #removeObsoleteFiles(retainPath?: string): Promise<void> {
    const entries = await readdir(this.#directory);
    await Promise.all(
      entries
        .filter(
          (entry) =>
            /^artwork-\d+(?:-.+)?\.jpg$/.test(entry) ||
            /^\.artwork-\d+(?:-.+)?\.jpg\..+\.tmp$/.test(entry),
        )
        .map((entry) => path.join(this.#directory, entry))
        .filter((entryPath) => entryPath !== retainPath)
        .map(removeIfPresent),
    );
  }
}

async function removeIfPresent(filePath: string): Promise<void> {
  try {
    await unlink(filePath);
  } catch (error) {
    if (!isMissingFile(error)) {
      throw error;
    }
  }
}

function isMissingFile(error: unknown): boolean {
  return (
    error instanceof Error &&
    "code" in error &&
    (error as NodeJS.ErrnoException).code === "ENOENT"
  );
}
