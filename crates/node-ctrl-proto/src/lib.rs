// Copyright (c) 2023 -  Restate Software, Inc., Restate GmbH.
// All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod proto {
    #![allow(warnings)]
    #![allow(clippy::all)]
    #![allow(unknown_lints)]
    tonic::include_proto!("dev.restate.node_ctrl");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("node_ctrl_descriptor");
}

pub mod cluster_controller {
    #![allow(warnings)]
    #![allow(clippy::all)]
    #![allow(unknown_lints)]

    tonic::include_proto!("dev.restate.cluster_controller");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("cluster_controller_descriptor");

    impl From<NodeId> for restate_types::NodeId {
        fn from(node_id: NodeId) -> Self {
            restate_types::NodeId::new(node_id.id, node_id.generation)
        }
    }

    impl From<restate_types::NodeId> for NodeId {
        fn from(node_id: restate_types::NodeId) -> Self {
            NodeId {
                id: node_id.id().into(),
                generation: node_id.as_generational().map(|g| g.generation()),
            }
        }
    }
}
