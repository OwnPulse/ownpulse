// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

#import "ObjCExceptionCatcher.h"

static NSString *const OwnPulseObjCExceptionDomain = @"OwnPulseObjCException";

@implementation ObjCExceptionCatcher

+ (BOOL)tryBlock:(void (NS_NOESCAPE ^)(void))block error:(NSError **)error {
    @try {
        block();
        return YES;
    } @catch (NSException *exception) {
        if (error) {
            NSMutableDictionary *userInfo = [NSMutableDictionary dictionary];
            if (exception.name) {
                userInfo[@"ExceptionName"] = exception.name;
            }
            if (exception.reason) {
                userInfo[NSLocalizedDescriptionKey] = exception.reason;
                userInfo[@"ExceptionReason"] = exception.reason;
            }
            if (exception.callStackSymbols) {
                userInfo[@"CallStackSymbols"] = exception.callStackSymbols;
            }
            *error = [NSError errorWithDomain:OwnPulseObjCExceptionDomain
                                         code:0
                                     userInfo:userInfo];
        }
        return NO;
    }
}

@end
