class StubDatabase {
  constructor() {
    throw new Error("sqlite3 is not available in this build");
  }
}

const stub = { Database: StubDatabase };
export default stub;
export { StubDatabase as Database };
