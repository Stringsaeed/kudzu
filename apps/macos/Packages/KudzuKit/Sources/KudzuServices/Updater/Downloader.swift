import Foundation

/// Downloads a URL into memory, reporting progress per received chunk (`URLSessionDataDelegate`).
final class Downloader: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    // @unchecked: the delegate callbacks run on the session's serial delegate queue; `data`,
    // `total` and `continuation` are only touched there (and once before `resume`).
    private var data = Data()
    private var total: UInt64?
    private var continuation: CheckedContinuation<Data, Error>?
    private let progress: @Sendable (_ downloaded: UInt64, _ total: UInt64?) -> Void
    private let limit: Int64

    static let tooLarge = ServiceError("The update is larger than expected. It was not downloaded.")

    private init(limit: Int64, progress: @escaping @Sendable (UInt64, UInt64?) -> Void) {
        self.limit = limit
        self.progress = progress
    }

    /// Fetches `url` into memory, failing once more than `limit` bytes arrive (or are announced).
    static func fetch(_ url: URL, limit: Int64 = Installer.maxArchiveBytes, progress: @escaping @Sendable (UInt64, UInt64?) -> Void) async throws -> Data {
        let d = Downloader(limit: limit, progress: progress)
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        let session = URLSession(configuration: .ephemeral, delegate: d, delegateQueue: queue)
        defer { session.finishTasksAndInvalidate() }
        let request: URLRequest = {
            var r = URLRequest(url: url)
            r.setValue("Kudzu-Updater", forHTTPHeaderField: "User-Agent")
            return r
        }()
        return try await withCheckedThrowingContinuation { c in
            queue.addOperation {
                d.continuation = c
                session.dataTask(with: request).resume()
            }
        }
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        if let http = response as? HTTPURLResponse, !(200 ..< 300).contains(http.statusCode) {
            completionHandler(.cancel)
            finish(.failure(ServiceError("Download request failed with status: \(http.statusCode)")))
            return
        }
        // The URL comes from the unsigned feed: never trust its size (nor reserve memory for it).
        if response.expectedContentLength > limit {
            completionHandler(.cancel)
            finish(.failure(Self.tooLarge))
            return
        }
        total = response.expectedContentLength > 0 ? UInt64(response.expectedContentLength) : nil
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive chunk: Data) {
        guard continuation != nil else { return }
        guard Int64(data.count) + Int64(chunk.count) <= limit else {
            dataTask.cancel()
            data = Data()
            finish(.failure(Self.tooLarge))
            return
        }
        data.append(chunk)
        progress(UInt64(data.count), total)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error { finish(.failure(error)) } else { finish(.success(data)) }
    }

    private func finish(_ result: Result<Data, Error>) {
        guard let c = continuation else { return }
        continuation = nil
        c.resume(with: result)
    }
}
