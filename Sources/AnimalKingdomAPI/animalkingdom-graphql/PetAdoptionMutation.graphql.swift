// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class PetAdoptionMutation: GraphQLMutation {
  public static let operationName: String = "PetAdoptionMutation"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation PetAdoptionMutation($input: PetAdoptionInput!) { adoptPet(input: $input) { __typename id humanName } }"#
    ))

  public var input: TestSchema.PetAdoptionInput

  public init(input: TestSchema.PetAdoptionInput) {
    self.input = input
  }

  public var __variables: Variables? { ["input": input] }

  public struct Data: TestSchema.SelectionSet {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Mutation }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("adoptPet", AdoptPet.self, arguments: ["input": .variable("input")]),
    ] }

    public var adoptPet: AdoptPet { __data["adoptPet"] }

    /// AdoptPet
    public struct AdoptPet: TestSchema.SelectionSet {
      public let __data: DataDict
      public init(_dataDict: DataDict) { __data = _dataDict }

      public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
      public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("humanName", String?.self),
      ] }

      public var id: TestSchema.ID { __data["id"] }
      public var humanName: String? { __data["humanName"] }
    }
  }
}
