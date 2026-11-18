// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian

use std::io;
use std::net::Ipv4Addr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Lan {
    pub address: Ipv4Addr,
    mask: u32,
    #[cfg(windows)]
    interface: u32,
    #[cfg(windows)]
    network: windows::core::GUID,
}

impl Lan {
    pub fn contains(self, peer: Ipv4Addr) -> bool {
        #[cfg(test)]
        if self.address.is_loopback() {
            return peer.is_loopback();
        }
        peer.is_private()
            && peer != self.address
            && u32::from(peer) & self.mask == u32::from(self.address) & self.mask
    }

    pub fn is_current(self) -> bool {
        #[cfg(test)]
        if self.address.is_loopback() {
            return true;
        }
        lan(self.address).is_ok_and(|current| current == self)
    }

    #[cfg(test)]
    pub fn loopback() -> Self {
        Self {
            address: Ipv4Addr::LOCALHOST,
            mask: u32::MAX,
            #[cfg(windows)]
            interface: 0,
            #[cfg(windows)]
            network: windows::core::GUID::zeroed(),
        }
    }
}

fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::AddrNotAvailable,
        "Conecte-se a uma rede local privada por Wi-Fi ou Ethernet.",
    )
}

#[cfg(windows)]
pub(super) fn local_address() -> io::Result<Ipv4Addr> {
    use std::net::{SocketAddr, UdpSocket};

    // UDP connect asks Windows which source address it would use, without sending packets.
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9))?;
    let SocketAddr::V4(address) = socket.local_addr()? else {
        return Err(unavailable());
    };
    lan(*address.ip()).map(|lan| lan.address)
}

#[cfg(windows)]
pub(super) fn lan(address: Ipv4Addr) -> io::Result<Lan> {
    use std::ptr;
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{IpDadStatePreferred, AF_INET};

    if !address.is_private() {
        return Err(unavailable());
    }
    let mut table = ptr::null_mut();
    // The system allocates the table and returns it through this initialized out parameter.
    let status = unsafe { GetUnicastIpAddressTable(AF_INET, &mut table) };
    if status.0 != 0 || table.is_null() {
        return Err(unavailable());
    }
    let result = (|| {
        // The API promises NumEntries initialized entries in its trailing array. AF_INET
        // restricts the entries to IPv4 and makes reading the IPv4 sockaddr union arm valid.
        let rows = unsafe {
            std::slice::from_raw_parts(
                ptr::addr_of!((*table).Table).cast::<MIB_UNICASTIPADDRESS_ROW>(),
                (*table).NumEntries as usize,
            )
        };
        let row = rows
            .iter()
            .find(|row| {
                let ipv4 = unsafe { row.Address.Ipv4.sin_addr.S_un.S_addr };
                Ipv4Addr::from(ipv4.to_ne_bytes()) == address && row.DadState == IpDadStatePreferred
            })
            .ok_or_else(unavailable)?;
        let prefix = row.OnLinkPrefixLength;
        if !(1..=31).contains(&prefix) {
            return Err(unavailable());
        }
        let mut interface = MIB_IF_ROW2 {
            InterfaceIndex: row.InterfaceIndex,
            ..Default::default()
        };
        // InterfaceIndex is from a live unicast table entry and the out buffer is initialized.
        let status = unsafe { GetIfEntry2(&mut interface) };
        // MIB_IF_ROW2 bit 0 is HardwareInterface. This excludes software-only network adapters.
        let hardware = interface.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
        if status.0 != 0
            || !hardware
            || interface.OperStatus != IfOperStatusUp
            || !matches!(interface.Type, IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211)
        {
            return Err(unavailable());
        }
        Ok(Lan {
            address,
            mask: u32::MAX << (32 - prefix),
            interface: row.InterfaceIndex,
            network: interface.NetworkGuid,
        })
    })();
    // GetUnicastIpAddressTable owns this allocation and requires FreeMibTable.
    unsafe { FreeMibTable(table.cast()) };
    result
}

#[cfg(not(windows))]
pub(super) fn local_address() -> io::Result<Ipv4Addr> {
    Err(unavailable())
}

#[cfg(not(windows))]
pub(super) fn lan(_: Ipv4Addr) -> io::Result<Lan> {
    Err(unavailable())
}
