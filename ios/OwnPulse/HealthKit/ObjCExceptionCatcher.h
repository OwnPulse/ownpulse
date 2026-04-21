// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// Bridges Objective-C `@try/@catch` into Swift.
///
/// HealthKit's `HKHealthStore.requestAuthorization` raises `NSException`
/// (not `NSError`) when it decides a requested type is disallowed for
/// sharing or is malformed. Swift cannot catch Objective-C exceptions,
/// so the process crashes with `SIGABRT` rather than returning an error
/// the app can handle. Wrap the offending call in `ObjCExceptionCatcher.try`
/// to convert an uncaught `NSException` into an `NSError` the Swift side
/// can deal with.
@interface ObjCExceptionCatcher : NSObject

/// Executes `block` inside an `@try/@catch`. If the block raises an
/// `NSException`, returns `NO` and populates `error` with an `NSError` in
/// the `OwnPulseObjCException` domain whose `userInfo` preserves the
/// original exception's name, reason, and `callStackSymbols`.
+ (BOOL)tryBlock:(void (NS_NOESCAPE ^)(void))block error:(NSError * _Nullable __autoreleasing * _Nullable)error;

@end

NS_ASSUME_NONNULL_END
