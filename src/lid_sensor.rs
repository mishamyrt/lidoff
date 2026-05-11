use core_foundation::base::{CFRelease, CFRetain, CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionary};
use core_foundation::number::CFNumber;
use core_foundation::set::{CFSet, CFSetRef};
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFIndex, CFTypeRef, kCFAllocatorDefault};
use core_foundation_sys::set::CFSetGetValues;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::{Mutex, OnceLock};

const K_IO_HID_VENDOR_ID_KEY: &str = "VendorID";
const K_IO_HID_PRODUCT_ID_KEY: &str = "ProductID";
const K_IO_HID_USAGE_PAGE_KEY: &str = "UsagePage";
const K_IO_HID_USAGE_KEY: &str = "Usage";

const LID_SENSOR_VENDOR_ID: i32 = 0x05AC;
const LID_SENSOR_PRODUCT_ID: i32 = 0x8104;
const LID_SENSOR_USAGE_PAGE: i32 = 0x0020;
const LID_SENSOR_USAGE: i32 = 0x008A;

const PRIMARY_REPORT_ID: CFIndex = 1;
const FALLBACK_REPORT_ID: CFIndex = 0;
const REPORT_LEN: usize = 8;

pub const LID_ANGLE_ERROR: i32 = -1;

type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOReturn = c_int;

struct LidSensorConnection {
    manager: IOHIDManagerRef,
    device: IOHIDDeviceRef,
    report_id: CFIndex,
}

// CoreFoundation/IOKit refs stay behind a mutex and are only touched through this module.
unsafe impl Send for LidSensorConnection {}

#[repr(C)]
enum IOHIDReportType {
    Feature = 2,
}

#[repr(C)]
enum IOHIDOptionsType {
    None = 0,
}

unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: CFTypeRef, options: IOHIDOptionsType) -> IOHIDManagerRef;
    fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFTypeRef;
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: IOHIDOptionsType) -> IOReturn;
    fn IOHIDManagerClose(manager: IOHIDManagerRef, options: IOHIDOptionsType) -> IOReturn;

    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: IOHIDOptionsType) -> IOReturn;
    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: IOHIDOptionsType) -> IOReturn;
    fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        report_type: IOHIDReportType,
        report_id: CFIndex,
        report: *mut u8,
        report_length: *mut CFIndex,
    ) -> IOReturn;
}

static LID_SENSOR: OnceLock<Mutex<Option<LidSensorConnection>>> = OnceLock::new();

pub fn init() -> bool {
    let Some(connection) = find_lid_sensor() else {
        return false;
    };

    let mut guard = lock_sensor();
    if let Some(existing) = guard.replace(connection) {
        drop_connection(existing);
    }
    true
}

pub fn close() {
    let mut guard = lock_sensor();
    if let Some(connection) = guard.take() {
        drop_connection(connection);
    }
}

pub fn get_angle() -> i32 {
    let guard = lock_sensor();
    let Some(connection) = guard.as_ref() else {
        return LID_ANGLE_ERROR;
    };

    let Some(report) = read_report(connection.device, connection.report_id) else {
        return LID_ANGLE_ERROR;
    };

    parse_angle(report)
}

fn create_matching_dictionary() -> CFDictionary<CFType, CFType> {
    let mut dictionary = CFMutableDictionary::new();

    dictionary.set(
        CFString::from_static_string(K_IO_HID_VENDOR_ID_KEY).as_CFType(),
        CFNumber::from(LID_SENSOR_VENDOR_ID).as_CFType(),
    );
    dictionary.set(
        CFString::from_static_string(K_IO_HID_PRODUCT_ID_KEY).as_CFType(),
        CFNumber::from(LID_SENSOR_PRODUCT_ID).as_CFType(),
    );
    dictionary.set(
        CFString::from_static_string(K_IO_HID_USAGE_PAGE_KEY).as_CFType(),
        CFNumber::from(LID_SENSOR_USAGE_PAGE).as_CFType(),
    );
    dictionary.set(
        CFString::from_static_string(K_IO_HID_USAGE_KEY).as_CFType(),
        CFNumber::from(LID_SENSOR_USAGE).as_CFType(),
    );

    dictionary.to_immutable()
}

fn find_lid_sensor() -> Option<LidSensorConnection> {
    unsafe {
        let manager = IOHIDManagerCreate(kCFAllocatorDefault, IOHIDOptionsType::None);
        if manager.is_null() {
            return None;
        }

        let matching_dictionary = create_matching_dictionary();
        IOHIDManagerSetDeviceMatching(manager, matching_dictionary.as_concrete_TypeRef());

        if IOHIDManagerOpen(manager, IOHIDOptionsType::None) != 0 {
            CFRelease(manager as CFTypeRef);
            return None;
        }

        let devices_set_ref = IOHIDManagerCopyDevices(manager);
        if devices_set_ref.is_null() {
            IOHIDManagerClose(manager, IOHIDOptionsType::None);
            CFRelease(manager as CFTypeRef);
            return None;
        }

        let devices_set: CFSet<*const c_void> =
            CFSet::wrap_under_create_rule(devices_set_ref as CFSetRef);
        let mut device_values = vec![ptr::null(); devices_set.len()];
        CFSetGetValues(
            devices_set.as_concrete_TypeRef(),
            device_values.as_mut_ptr(),
        );

        for device_value in device_values {
            if device_value.is_null() {
                continue;
            }

            let device = device_value as IOHIDDeviceRef;
            if IOHIDDeviceOpen(device, IOHIDOptionsType::None) != 0 {
                continue;
            }

            if let Some(report_id) = detect_report_id(device) {
                CFRetain(device as CFTypeRef);
                return Some(LidSensorConnection {
                    manager,
                    device,
                    report_id,
                });
            }

            IOHIDDeviceClose(device, IOHIDOptionsType::None);
        }

        IOHIDManagerClose(manager, IOHIDOptionsType::None);
        CFRelease(manager as CFTypeRef);
        None
    }
}

fn detect_report_id(device: IOHIDDeviceRef) -> Option<CFIndex> {
    [PRIMARY_REPORT_ID, FALLBACK_REPORT_ID]
        .into_iter()
        .find(|&report_id| read_report(device, report_id).is_some())
}

fn read_report(device: IOHIDDeviceRef, report_id: CFIndex) -> Option<[u8; REPORT_LEN]> {
    unsafe {
        let mut report = [0_u8; REPORT_LEN];
        let mut report_length = report.len() as CFIndex;
        let result = IOHIDDeviceGetReport(
            device,
            IOHIDReportType::Feature,
            report_id,
            report.as_mut_ptr(),
            &mut report_length,
        );

        if result != 0 || report_length < 3 {
            return None;
        }

        Some(report)
    }
}

fn parse_angle(report: [u8; REPORT_LEN]) -> i32 {
    let angle = i32::from(u16::from_le_bytes([report[1], report[2]]));
    if (0..=180).contains(&angle) {
        angle
    } else {
        LID_ANGLE_ERROR
    }
}

fn drop_connection(connection: LidSensorConnection) {
    unsafe {
        IOHIDDeviceClose(connection.device, IOHIDOptionsType::None);
        CFRelease(connection.device as CFTypeRef);
        IOHIDManagerClose(connection.manager, IOHIDOptionsType::None);
        CFRelease(connection.manager as CFTypeRef);
    }
}

fn lock_sensor() -> std::sync::MutexGuard<'static, Option<LidSensorConnection>> {
    let mutex = LID_SENSOR.get_or_init(|| Mutex::new(None));
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
